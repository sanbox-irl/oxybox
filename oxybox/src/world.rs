use glam::Vec2;
use oxybox_sys::{self as sys, b2DestroyBody, b2DestroyWorld};

use crate::{Body, BodyId, ShapeId};

pub struct World {
    id: WorldId,
    dt: f32,
}

impl World {
    const SUBSTEPS: i32 = 4;

    pub fn new(delta_time: f32) -> Self {
        let id = unsafe { WorldId(sys::b2CreateWorld(&sys::b2DefaultWorldDef())) };
        Self { id, dt: delta_time }
    }

    pub fn id(&self) -> WorldId {
        self.id
    }

    pub fn body(&self, body_id: BodyId) -> Body {
        let (body_id, shape_id) = unsafe {
            let count = sys::b2Body_GetShapeCount(*body_id);
            assert_eq!(count, 1, "oxybox can only handle 1 shape per body right now");
            let mut vec = Vec::with_capacity(count as usize);
            sys::b2Body_GetShapes(*body_id, vec.as_mut_ptr(), count);
            vec.set_len(count as usize);
            (body_id, ShapeId::from(vec[0]))
        };
        Body::new(body_id, shape_id, self.id)
    }

    pub fn step(&self) {
        unsafe {
            sys::b2World_Step(*self.id, self.dt, Self::SUBSTEPS);
        }
    }

    pub fn set_gravity(&self, gravity: Vec2) {
        unsafe { sys::b2World_SetGravity(*self.id, gravity.into()) }
    }

    /// Sets the pixels-per-meter Box2D will expect. While you're free to work with whatever units
    /// you please, setting this will help Box2D tweak internal numbers to better work with your
    /// expectations.
    ///
    /// **NOTE: This is a global value -- Box2D does not support different unit lengths per-world.**
    pub fn set_length_units_per_meter(&self, ppm: f32) {
        unsafe { sys::b2SetLengthUnitsPerMeter(ppm) }
    }

    pub fn length_units_per_meter(&self) -> f32 {
        unsafe { sys::b2GetLengthUnitsPerMeter() }
    }

    pub fn destroy_body(&self, body: Body) {
        unsafe {
            b2DestroyBody(*body.body_id());
        }
    }

    pub fn body_valid(&self, body_id: BodyId) -> bool {
        unsafe { sys::b2Body_IsValid(*body_id) }
    }

    pub fn shape_valid(&self, shape_id: ShapeId) -> bool {
        unsafe { sys::b2Shape_IsValid(*shape_id) }
    }

    pub fn contact_events(&self) -> impl Iterator<Item = (BodyId, BodyId)> {
        unsafe {
            let contact_events = sys::b2World_GetContactEvents(*self.id);
            let begin_events: &mut [sys::b2ContactBeginTouchEvent] =
                std::slice::from_raw_parts_mut(contact_events.beginEvents, contact_events.beginCount as usize);

            begin_events.iter_mut().filter_map(|e| {
                if !sys::b2Shape_IsValid(e.shapeIdA) || !sys::b2Shape_IsValid(e.shapeIdB) {
                    None
                } else {
                    Some((
                        sys::b2Shape_GetBody(e.shapeIdA).into(),
                        sys::b2Shape_GetBody(e.shapeIdB).into(),
                    ))
                }
            })
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        unsafe { b2DestroyWorld(*self.id) }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct WorldId(sys::b2WorldId);

impl PartialEq for WorldId {
    fn eq(&self, other: &Self) -> bool {
        self.0.generation == other.0.generation && self.0.index1 == other.0.index1
    }
}

impl Eq for WorldId {}

impl std::hash::Hash for WorldId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let bits: u32 = unsafe { std::mem::transmute(self.0) };
        bits.hash(state);
    }
}

impl std::ops::Deref for WorldId {
    type Target = sys::b2WorldId;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
