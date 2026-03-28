use arachne_math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch {
    pub id: u64,
    pub position: Vec2,
    pub phase: TouchPhase,
}

#[derive(Clone, Debug, Default)]
pub struct TouchState {
    touches: Vec<Touch>,
}

impl TouchState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process_touch(&mut self, id: u64, position: Vec2, phase: TouchPhase) {
        if let Some(existing) = self.touches.iter_mut().find(|t| t.id == id) {
            existing.position = position;
            existing.phase = phase;
        } else if matches!(phase, TouchPhase::Started | TouchPhase::Moved) {
            if self.touches.len() < 10 {
                self.touches.push(Touch { id, position, phase });
            }
        }
    }

    pub fn active_touches(&self) -> &[Touch] {
        &self.touches
    }

    pub fn touch_count(&self) -> usize {
        self.touches.len()
    }

    pub fn get_touch(&self, id: u64) -> Option<&Touch> {
        self.touches.iter().find(|t| t.id == id)
    }

    pub fn any_touch_active(&self) -> bool {
        self.touches.iter().any(|t| {
            matches!(t.phase, TouchPhase::Started | TouchPhase::Moved)
        })
    }

    pub fn begin_frame(&mut self) {
        self.touches.retain(|t| {
            !matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_start_move_end_lifecycle() {
        let mut touch = TouchState::new();

        touch.process_touch(1, Vec2::new(100.0, 200.0), TouchPhase::Started);
        assert_eq!(touch.touch_count(), 1);
        assert_eq!(touch.get_touch(1).unwrap().phase, TouchPhase::Started);

        touch.process_touch(1, Vec2::new(110.0, 210.0), TouchPhase::Moved);
        assert_eq!(touch.touch_count(), 1);
        assert_eq!(touch.get_touch(1).unwrap().position, Vec2::new(110.0, 210.0));
        assert_eq!(touch.get_touch(1).unwrap().phase, TouchPhase::Moved);

        touch.process_touch(1, Vec2::new(110.0, 210.0), TouchPhase::Ended);
        assert_eq!(touch.get_touch(1).unwrap().phase, TouchPhase::Ended);

        touch.begin_frame();
        assert_eq!(touch.touch_count(), 0);
    }

    #[test]
    fn multi_touch_three_fingers() {
        let mut touch = TouchState::new();

        touch.process_touch(1, Vec2::new(100.0, 100.0), TouchPhase::Started);
        touch.process_touch(2, Vec2::new(200.0, 200.0), TouchPhase::Started);
        touch.process_touch(3, Vec2::new(300.0, 300.0), TouchPhase::Started);

        assert_eq!(touch.touch_count(), 3);
        assert_eq!(touch.get_touch(1).unwrap().id, 1);
        assert_eq!(touch.get_touch(2).unwrap().id, 2);
        assert_eq!(touch.get_touch(3).unwrap().id, 3);
    }

    #[test]
    fn touch_ids_stable_across_frames() {
        let mut touch = TouchState::new();

        touch.process_touch(42, Vec2::new(50.0, 50.0), TouchPhase::Started);
        touch.begin_frame();
        touch.process_touch(42, Vec2::new(60.0, 60.0), TouchPhase::Moved);

        assert_eq!(touch.touch_count(), 1);
        let t = touch.get_touch(42).unwrap();
        assert_eq!(t.id, 42);
        assert_eq!(t.position, Vec2::new(60.0, 60.0));
    }

    #[test]
    fn max_ten_simultaneous() {
        let mut touch = TouchState::new();
        for i in 0..12 {
            touch.process_touch(i, Vec2::new(i as f32, 0.0), TouchPhase::Started);
        }
        assert_eq!(touch.touch_count(), 10);
    }

    #[test]
    fn cancelled_removed_on_frame() {
        let mut touch = TouchState::new();
        touch.process_touch(1, Vec2::new(0.0, 0.0), TouchPhase::Started);
        touch.process_touch(1, Vec2::new(0.0, 0.0), TouchPhase::Cancelled);
        touch.begin_frame();
        assert_eq!(touch.touch_count(), 0);
    }

    #[test]
    fn any_touch_active() {
        let mut touch = TouchState::new();
        assert!(!touch.any_touch_active());

        touch.process_touch(1, Vec2::new(0.0, 0.0), TouchPhase::Started);
        assert!(touch.any_touch_active());

        touch.process_touch(1, Vec2::new(0.0, 0.0), TouchPhase::Ended);
        assert!(!touch.any_touch_active());
    }
}
