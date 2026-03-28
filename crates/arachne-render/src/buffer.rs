use std::marker::PhantomData;

// ---------------------------------------------------------------------------
// BufferPool: reusable GPU buffers
// ---------------------------------------------------------------------------

struct PooledBuffer {
    buffer: wgpu::Buffer,
    size: u64,
    usage: wgpu::BufferUsages,
}

/// A pool of reusable GPU buffers. Return buffers at frame end; reuse next frame.
pub struct BufferPool {
    available: Vec<PooledBuffer>,
    in_use: Vec<PooledBuffer>,
}

/// Handle to a buffer allocated from the pool.
pub struct BufferAllocation {
    index: usize,
}

impl BufferPool {
    pub fn new() -> Self {
        Self {
            available: Vec::new(),
            in_use: Vec::new(),
        }
    }

    /// Allocate a buffer of at least `size` bytes. Reuses a pooled buffer if one fits.
    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        size: u64,
        usage: wgpu::BufferUsages,
    ) -> (BufferAllocation, &wgpu::Buffer) {
        // Find a reusable buffer that is large enough and has matching usage
        let found = self
            .available
            .iter()
            .position(|b| b.size >= size && b.usage == usage);

        if let Some(idx) = found {
            let buf = self.available.swap_remove(idx);
            self.in_use.push(buf);
        } else {
            // Round up to power of 2 for fewer reallocations (min 256 bytes)
            let alloc_size = size.max(256).next_power_of_two();
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pool_buffer"),
                size: alloc_size,
                usage,
                mapped_at_creation: false,
            });
            self.in_use.push(PooledBuffer {
                buffer,
                size: alloc_size,
                usage,
            });
        }

        let index = self.in_use.len() - 1;
        let alloc = BufferAllocation { index };
        (alloc, &self.in_use[index].buffer)
    }

    /// Get the wgpu::Buffer for an allocation.
    pub fn get(&self, alloc: &BufferAllocation) -> &wgpu::Buffer {
        &self.in_use[alloc.index].buffer
    }

    /// Return all in-use buffers to the available pool. Call at frame end.
    pub fn reset_frame(&mut self) {
        self.available.append(&mut self.in_use);
    }

    pub fn available_count(&self) -> usize {
        self.available.len()
    }

    pub fn in_use_count(&self) -> usize {
        self.in_use.len()
    }
}

// ---------------------------------------------------------------------------
// DynamicBuffer: growable GPU buffer with write support
// ---------------------------------------------------------------------------

/// A GPU buffer that grows as needed and supports data uploads.
pub struct DynamicBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
    len: u64,
    usage: wgpu::BufferUsages,
}

impl DynamicBuffer {
    pub fn new(device: &wgpu::Device, initial_capacity: u64, usage: wgpu::BufferUsages) -> Self {
        let capacity = initial_capacity.max(64);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dynamic_buffer"),
            size: capacity,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            capacity,
            len: 0,
            usage: usage | wgpu::BufferUsages::COPY_DST,
        }
    }

    /// Write data to the buffer, growing if necessary. Returns byte offset where data was written.
    pub fn write<T: bytemuck::Pod>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[T],
    ) -> u64 {
        let bytes = bytemuck::cast_slice(data);
        let byte_len = bytes.len() as u64;
        let offset = self.len;

        if offset + byte_len > self.capacity {
            // Grow: double capacity or fit data, whichever is larger
            let new_capacity = (self.capacity * 2).max(offset + byte_len).next_power_of_two();
            let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("dynamic_buffer"),
                size: new_capacity,
                usage: self.usage,
                mapped_at_creation: false,
            });
            // Copy old data if any
            if self.len > 0 {
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                encoder.copy_buffer_to_buffer(&self.buffer, 0, &new_buffer, 0, self.len);
                queue.submit(std::iter::once(encoder.finish()));
            }
            self.buffer = new_buffer;
            self.capacity = new_capacity;
        }

        queue.write_buffer(&self.buffer, offset, bytes);
        self.len += byte_len;
        offset
    }

    /// Clear the logical length (reuse buffer memory next frame).
    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline]
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    #[inline]
    pub fn len(&self) -> u64 {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn capacity(&self) -> u64 {
        self.capacity
    }
}

// ---------------------------------------------------------------------------
// UniformBuffer<T>: typed uniform with automatic alignment
// ---------------------------------------------------------------------------

/// A typed GPU uniform buffer with associated bind group.
pub struct UniformBuffer<T: bytemuck::Pod> {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    _marker: PhantomData<T>,
}

impl<T: bytemuck::Pod> UniformBuffer<T> {
    pub fn new(device: &wgpu::Device, label: &str, initial: &T) -> Self {
        let size = std::mem::size_of::<T>() as u64;
        // wgpu requires uniform buffers to be at least 16-byte aligned
        let aligned_size = (size + 15) & !15;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: aligned_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });

        // Write initial data
        {
            let mut view = buffer.slice(..).get_mapped_range_mut();
            view[..size as usize].copy_from_slice(bytemuck::bytes_of(initial));
        }
        buffer.unmap();

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size),
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            bind_group,
            bind_group_layout,
            _marker: PhantomData,
        }
    }

    pub fn update(&self, queue: &wgpu::Queue, data: &T) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(data));
    }

    #[inline]
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    #[inline]
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    #[inline]
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

// ---------------------------------------------------------------------------
// VertexBuffer / IndexBuffer wrappers
// ---------------------------------------------------------------------------

/// A typed vertex buffer.
pub struct VertexBuffer<T: bytemuck::Pod> {
    buffer: wgpu::Buffer,
    count: u32,
    _marker: PhantomData<T>,
}

impl<T: bytemuck::Pod> VertexBuffer<T> {
    pub fn new(device: &wgpu::Device, data: &[T]) -> Self {
        use wgpu::util::DeviceExt;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex_buffer"),
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Self {
            buffer,
            count: data.len() as u32,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    #[inline]
    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn slice(&self) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(..)
    }
}

/// A GPU index buffer (u16 or u32 indices).
pub struct IndexBuffer {
    buffer: wgpu::Buffer,
    count: u32,
    format: wgpu::IndexFormat,
}

impl IndexBuffer {
    pub fn new_u16(device: &wgpu::Device, data: &[u16]) -> Self {
        use wgpu::util::DeviceExt;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index_buffer"),
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            buffer,
            count: data.len() as u32,
            format: wgpu::IndexFormat::Uint16,
        }
    }

    pub fn new_u32(device: &wgpu::Device, data: &[u32]) -> Self {
        use wgpu::util::DeviceExt;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index_buffer"),
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            buffer,
            count: data.len() as u32,
            format: wgpu::IndexFormat::Uint32,
        }
    }

    #[inline]
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    #[inline]
    pub fn count(&self) -> u32 {
        self.count
    }

    #[inline]
    pub fn format(&self) -> wgpu::IndexFormat {
        self.format
    }

    pub fn slice(&self) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(..)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device() -> (wgpu::Device, wgpu::Queue) {
        let ctx = pollster::block_on(crate::context::RenderContext::new_headless()).unwrap();
        // We need owned device/queue, but RenderContext owns them.
        // For tests, create directly.
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    ..Default::default()
                })
                .await
                .unwrap();
            adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .unwrap()
        })
    }

    #[test]
    fn buffer_pool_allocate_and_reuse() {
        let (device, _queue) = test_device();
        let mut pool = BufferPool::new();

        // Allocate
        let (alloc1, _buf1) = pool.allocate(
            &device,
            128,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        assert_eq!(pool.in_use_count(), 1);
        assert_eq!(pool.available_count(), 0);

        // Reset frame -> buffer returns to pool
        pool.reset_frame();
        assert_eq!(pool.in_use_count(), 0);
        assert_eq!(pool.available_count(), 1);

        // Reallocate -> should reuse
        let (alloc2, _buf2) = pool.allocate(
            &device,
            64, // smaller, should still reuse the 128+ byte buffer
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        assert_eq!(pool.in_use_count(), 1);
        assert_eq!(pool.available_count(), 0);
    }

    #[test]
    fn dynamic_buffer_write_and_grow() {
        let (device, queue) = test_device();
        let mut buf = DynamicBuffer::new(&device, 64, wgpu::BufferUsages::VERTEX);

        // Write some data
        let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let offset = buf.write(&device, &queue, &data);
        assert_eq!(offset, 0);
        assert_eq!(buf.len(), 16); // 4 * f32

        // Write more data
        let data2: Vec<f32> = vec![5.0, 6.0];
        let offset2 = buf.write(&device, &queue, &data2);
        assert_eq!(offset2, 16);
        assert_eq!(buf.len(), 24);

        // Clear and reuse
        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert!(buf.capacity() >= 64);
    }

    #[test]
    fn dynamic_buffer_grow_capacity() {
        let (device, queue) = test_device();
        let mut buf = DynamicBuffer::new(&device, 64, wgpu::BufferUsages::VERTEX);
        let initial_cap = buf.capacity();

        // Write data larger than initial capacity
        let big_data = vec![0u8; 200];
        buf.write(&device, &queue, &big_data);
        assert!(buf.capacity() > initial_cap);
        assert_eq!(buf.len(), 200);
    }

    #[test]
    fn uniform_buffer_create_and_update() {
        let (device, queue) = test_device();

        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct TestUniform {
            value: [f32; 4],
        }

        let initial = TestUniform {
            value: [1.0, 2.0, 3.0, 4.0],
        };
        let uniform = UniformBuffer::new(&device, "test_uniform", &initial);

        // Update
        let updated = TestUniform {
            value: [5.0, 6.0, 7.0, 8.0],
        };
        uniform.update(&queue, &updated);

        // Verify bind group exists
        let _bg = uniform.bind_group();
        let _bgl = uniform.bind_group_layout();
    }

    #[test]
    fn vertex_buffer_creation() {
        let (device, _queue) = test_device();

        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct Vertex {
            pos: [f32; 2],
        }

        let vertices = [
            Vertex { pos: [0.0, 0.0] },
            Vertex { pos: [1.0, 0.0] },
            Vertex { pos: [0.0, 1.0] },
        ];
        let vb = VertexBuffer::new(&device, &vertices);
        assert_eq!(vb.count(), 3);
    }

    #[test]
    fn index_buffer_creation() {
        let (device, _queue) = test_device();
        let indices: Vec<u16> = vec![0, 1, 2, 2, 3, 0];
        let ib = IndexBuffer::new_u16(&device, &indices);
        assert_eq!(ib.count(), 6);
        assert_eq!(ib.format(), wgpu::IndexFormat::Uint16);

        let indices32: Vec<u32> = vec![0, 1, 2];
        let ib32 = IndexBuffer::new_u32(&device, &indices32);
        assert_eq!(ib32.count(), 3);
        assert_eq!(ib32.format(), wgpu::IndexFormat::Uint32);
    }
}
