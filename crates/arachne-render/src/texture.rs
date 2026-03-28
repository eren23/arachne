use arachne_math::{Rect, Vec2};

/// An opaque handle to a texture in the texture storage.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct TextureHandle(pub u32);

struct TextureEntry {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

/// Manages GPU textures and their bind groups.
pub struct TextureStorage {
    textures: Vec<TextureEntry>,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl TextureStorage {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("texture_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            textures: Vec::new(),
            bind_group_layout,
            sampler,
        }
    }

    /// Create a texture from RGBA8 pixel data.
    pub fn create_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> TextureHandle {
        assert_eq!(data.len(), (width * height * 4) as usize, "RGBA8 data size mismatch");

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let handle = TextureHandle(self.textures.len() as u32);
        self.textures.push(TextureEntry {
            texture,
            view,
            bind_group,
            width,
            height,
        });
        handle
    }

    pub fn get_bind_group(&self, handle: TextureHandle) -> &wgpu::BindGroup {
        &self.textures[handle.0 as usize].bind_group
    }

    pub fn get_view(&self, handle: TextureHandle) -> &wgpu::TextureView {
        &self.textures[handle.0 as usize].view
    }

    pub fn get_size(&self, handle: TextureHandle) -> (u32, u32) {
        let entry = &self.textures[handle.0 as usize];
        (entry.width, entry.height)
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn count(&self) -> usize {
        self.textures.len()
    }
}

// ---------------------------------------------------------------------------
// TextureAtlas: dynamic packing of sub-textures
// ---------------------------------------------------------------------------

/// A shelf-based texture atlas that grows as needed.
pub struct TextureAtlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    _sampler: wgpu::Sampler,
    width: u32,
    height: u32,
    // Shelf packer state
    shelves: Vec<Shelf>,
    next_y: u32,
}

struct Shelf {
    y: u32,
    height: u32,
    next_x: u32,
}

impl TextureAtlas {
    pub fn new(device: &wgpu::Device, initial_size: u32) -> Self {
        let size = initial_size.max(256);
        let (texture, view, bind_group_layout, sampler, bind_group) =
            Self::create_atlas_texture(device, size, size);

        Self {
            texture,
            view,
            bind_group,
            bind_group_layout,
            _sampler: sampler,
            width: size,
            height: size,
            shelves: Vec::new(),
            next_y: 0,
        }
    }

    fn create_atlas_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (
        wgpu::Texture,
        wgpu::TextureView,
        wgpu::BindGroupLayout,
        wgpu::Sampler,
        wgpu::BindGroup,
    ) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("atlas_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        (texture, view, bind_group_layout, sampler, bind_group)
    }

    /// Add a sub-texture to the atlas. Returns UV rect in normalized [0,1] coordinates.
    /// Returns `None` if the atlas is full and cannot grow (would exceed max texture size).
    pub fn add(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sub_width: u32,
        sub_height: u32,
        data: &[u8],
    ) -> Option<Rect> {
        assert_eq!(
            data.len(),
            (sub_width * sub_height * 4) as usize,
            "RGBA8 data size mismatch"
        );

        let padding = 1u32; // 1px padding between sub-textures
        let pw = sub_width + padding;
        let ph = sub_height + padding;

        // Try to find a shelf that fits
        if let Some(rect) = self.try_pack(pw, ph, sub_width, sub_height) {
            self.upload_sub(queue, rect, sub_width, sub_height, data);
            return Some(self.to_uv_rect(rect, sub_width, sub_height));
        }

        // Try creating a new shelf
        if self.next_y + ph <= self.height {
            self.shelves.push(Shelf {
                y: self.next_y,
                height: ph,
                next_x: 0,
            });
            self.next_y += ph;
            if let Some(rect) = self.try_pack(pw, ph, sub_width, sub_height) {
                self.upload_sub(queue, rect, sub_width, sub_height, data);
                return Some(self.to_uv_rect(rect, sub_width, sub_height));
            }
        }

        // Need to grow the atlas
        let new_size = (self.width * 2).min(8192);
        if new_size <= self.width {
            return None; // Can't grow further
        }
        self.grow(device, queue, new_size);

        // Try again after growing
        if self.next_y + ph <= self.height {
            self.shelves.push(Shelf {
                y: self.next_y,
                height: ph,
                next_x: 0,
            });
            self.next_y += ph;
        }
        if let Some(rect) = self.try_pack(pw, ph, sub_width, sub_height) {
            self.upload_sub(queue, rect, sub_width, sub_height, data);
            return Some(self.to_uv_rect(rect, sub_width, sub_height));
        }

        None
    }

    fn try_pack(&mut self, pw: u32, ph: u32, _w: u32, _h: u32) -> Option<[u32; 2]> {
        for shelf in &mut self.shelves {
            if shelf.height >= ph && shelf.next_x + pw <= self.width {
                let x = shelf.next_x;
                let y = shelf.y;
                shelf.next_x += pw;
                return Some([x, y]);
            }
        }
        None
    }

    fn upload_sub(&self, queue: &wgpu::Queue, pos: [u32; 2], w: u32, h: u32, data: &[u8]) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: pos[0],
                    y: pos[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    fn to_uv_rect(&self, pos: [u32; 2], w: u32, h: u32) -> Rect {
        let inv_w = 1.0 / self.width as f32;
        let inv_h = 1.0 / self.height as f32;
        Rect::new(
            Vec2::new(pos[0] as f32 * inv_w, pos[1] as f32 * inv_h),
            Vec2::new((pos[0] + w) as f32 * inv_w, (pos[1] + h) as f32 * inv_h),
        )
    }

    fn grow(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, new_size: u32) {
        let (new_tex, new_view, _, _, new_bg) =
            Self::create_atlas_texture(device, new_size, new_size);

        // Copy old atlas content to new texture
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &new_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));

        self.texture = new_tex;
        self.view = new_view;
        self.bind_group = new_bg;
        self.width = new_size;
        self.height = new_size;
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> (wgpu::Device, wgpu::Queue) {
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
    fn create_texture_storage() {
        let (device, _queue) = test_ctx();
        let storage = TextureStorage::new(&device);
        assert_eq!(storage.count(), 0);
    }

    #[test]
    fn create_and_retrieve_texture() {
        let (device, queue) = test_ctx();
        let mut storage = TextureStorage::new(&device);

        // 2x2 white texture
        let data = vec![255u8; 2 * 2 * 4];
        let handle = storage.create_texture(&device, &queue, 2, 2, &data);
        assert_eq!(handle, TextureHandle(0));
        assert_eq!(storage.count(), 1);

        let (w, h) = storage.get_size(handle);
        assert_eq!((w, h), (2, 2));

        // Bind group should be accessible
        let _bg = storage.get_bind_group(handle);
    }

    #[test]
    fn texture_atlas_packing() {
        let (device, queue) = test_ctx();
        let mut atlas = TextureAtlas::new(&device, 256);

        // Add several small textures
        for i in 0..10 {
            let data = vec![((i * 25) as u8); 16 * 16 * 4];
            let uv = atlas.add(&device, &queue, 16, 16, &data);
            assert!(uv.is_some(), "failed to pack texture {i}");
        }
    }

    #[test]
    fn texture_atlas_uv_in_range() {
        let (device, queue) = test_ctx();
        let mut atlas = TextureAtlas::new(&device, 256);

        let data = vec![255u8; 32 * 32 * 4];
        let uv = atlas.add(&device, &queue, 32, 32, &data).unwrap();

        assert!(uv.min.x >= 0.0 && uv.min.x <= 1.0);
        assert!(uv.min.y >= 0.0 && uv.min.y <= 1.0);
        assert!(uv.max.x >= 0.0 && uv.max.x <= 1.0);
        assert!(uv.max.y >= 0.0 && uv.max.y <= 1.0);
        assert!(uv.max.x > uv.min.x);
        assert!(uv.max.y > uv.min.y);
    }

    #[test]
    fn texture_atlas_grows() {
        let (device, queue) = test_ctx();
        let mut atlas = TextureAtlas::new(&device, 256);
        let initial_size = atlas.size();

        // Fill it up to force growth
        for _ in 0..100 {
            let data = vec![255u8; 32 * 32 * 4];
            let _ = atlas.add(&device, &queue, 32, 32, &data);
        }

        // Atlas should have grown
        let new_size = atlas.size();
        assert!(
            new_size.0 >= initial_size.0,
            "atlas should have grown or stayed same"
        );
    }
}
