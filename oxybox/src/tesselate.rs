use bitflags::bitflags;
use glam::Vec2;
use oxybox_sys as sys;
use smol_rgb::EncodedColor;
use std::ffi::c_void;

use crate::WorldId;

#[derive(Debug, Clone)]
pub enum DrawCall {
    Circle {
        center: Vec2,
        radius: f32,
        color: EncodedColor,
        filled: bool,
    },
    Rect {
        center: Vec2,
        half_extents: Vec2,
        rotation: f32,
        color: EncodedColor,
        filled: bool,
    },
    Segment {
        p1: Vec2,
        p2: Vec2,
        color: EncodedColor,
    },
    Transform {
        position: Vec2,
        rotation: f32,
    },
    Point {
        position: Vec2,
        size: f32,
        color: EncodedColor,
    },
    String {
        position: Vec2,
        text: String,
        color: EncodedColor,
    },
    Capsule {
        center: Vec2,
        half_height: f32,
        radius: f32,
        rotation: f32,
        color: EncodedColor,
        filled: bool,
    },
}

bitflags! {
    pub struct TeselationFlags: u32 {
        const SHAPES             = 0b0000_0001;
        const JOINTS             = 0b0000_0010;
        const JOINT_EXTRAS       = 0b0000_0100;
        const BOUNDS             = 0b0000_1000;
        const MASS               = 0b0001_0000;
        const BODY_NAMES         = 0b0010_0000;
        const CONTACTS           = 0b0100_0000;
        const GRAPH_COLORS       = 0b1000_0000;
        const CONTACT_NORMALS    = 0b0001_0000_0000;
        const CONTACT_IMPULSES   = 0b0010_0000_0000;
        const CONTACT_FEATURES   = 0b0100_0000_0000;
        const FRICTION_IMPULSES  = 0b1000_0000_0000;
        const ISLANDS            = 0b0001_0000_0000_0000;
    }
}

#[inline]
fn b2_hex_to_encoded(c: u32) -> EncodedColor {
    let r = ((c >> 16) & 0xFF) as u8;
    let g = ((c >> 8) & 0xFF) as u8;
    let b = (c & 0xFF) as u8;
    EncodedColor::new(r, g, b, 255)
}

pub fn tesselate(world: WorldId, flags: TeselationFlags) -> Vec<DrawCall> {
    let mut calls = Vec::new();

    let mut dd = sys::b2DebugDraw {
        DrawCircleFcn: Some(draw_circle_cb),
        DrawSolidCircleFcn: Some(draw_solid_circle_cb),
        DrawPolygonFcn: Some(draw_polygon_cb),
        DrawSolidPolygonFcn: Some(draw_solid_polygon_cb),
        DrawSolidCapsuleFcn: Some(draw_solid_capsule_cb_compat),
        DrawSegmentFcn: Some(draw_segment_cb),
        DrawTransformFcn: Some(draw_transform_cb),
        DrawPointFcn: Some(draw_point_cb),
        DrawStringFcn: Some(draw_string_cb),

        drawShapes: flags.contains(TeselationFlags::SHAPES),
        drawJoints: flags.contains(TeselationFlags::JOINTS),
        drawJointExtras: flags.contains(TeselationFlags::JOINT_EXTRAS),
        drawBounds: flags.contains(TeselationFlags::BOUNDS),
        drawMass: flags.contains(TeselationFlags::MASS),
        drawBodyNames: flags.contains(TeselationFlags::BODY_NAMES),
        drawContacts: flags.contains(TeselationFlags::CONTACTS),
        drawGraphColors: flags.contains(TeselationFlags::GRAPH_COLORS),
        drawContactNormals: flags.contains(TeselationFlags::CONTACT_NORMALS),
        drawContactImpulses: flags.contains(TeselationFlags::CONTACT_IMPULSES),
        drawContactFeatures: flags.contains(TeselationFlags::CONTACT_FEATURES),
        drawFrictionImpulses: flags.contains(TeselationFlags::FRICTION_IMPULSES),
        drawIslands: flags.contains(TeselationFlags::ISLANDS),
        drawingBounds: sys::b2AABB {
            lowerBound: sys::b2Vec2 {
                x: -100000.0,
                y: -100000.0,
            },
            upperBound: sys::b2Vec2 {
                x: 100000.0,
                y: 100000.0,
            }, // todo
        },

        context: &mut calls as *mut _ as *mut c_void,
    };

    unsafe {
        sys::b2World_Draw(*world, &mut dd as *mut _);
    }

    calls
}

extern "C" fn draw_circle_cb(center: sys::b2Vec2, radius: f32, color: sys::b2HexColor, ctx: *mut c_void) {
    let calls = unsafe { &mut *(ctx as *mut Vec<DrawCall>) };
    calls.push(DrawCall::Circle {
        center: Vec2::new(center.x, center.y),
        radius,
        color: b2_hex_to_encoded(color),
        filled: false,
    });
}

extern "C" fn draw_solid_circle_cb(transform: sys::b2Transform, radius: f32, color: sys::b2HexColor, ctx: *mut c_void) {
    let calls = unsafe { &mut *(ctx as *mut Vec<DrawCall>) };
    let center = transform.p;
    calls.push(DrawCall::Circle {
        center: Vec2::new(center.x, center.y),
        radius,
        color: b2_hex_to_encoded(color),
        filled: true,
    });
}

extern "C" fn draw_segment_cb(p1: sys::b2Vec2, p2: sys::b2Vec2, color: sys::b2HexColor, ctx: *mut c_void) {
    let calls = unsafe { &mut *(ctx as *mut Vec<DrawCall>) };
    calls.push(DrawCall::Segment {
        p1: Vec2::new(p1.x, p1.y),
        p2: Vec2::new(p2.x, p2.y),
        color: b2_hex_to_encoded(color),
    });
}

extern "C" fn draw_transform_cb(transform: sys::b2Transform, ctx: *mut c_void) {
    let calls = unsafe { &mut *(ctx as *mut Vec<DrawCall>) };
    let pos = transform.p;
    let rot = transform.q.s.atan2(transform.q.c);
    calls.push(DrawCall::Transform {
        position: Vec2::new(pos.x, pos.y),
        rotation: rot,
    });
}

extern "C" fn draw_point_cb(position: sys::b2Vec2, size: f32, color: sys::b2HexColor, ctx: *mut c_void) {
    let calls = unsafe { &mut *(ctx as *mut Vec<DrawCall>) };
    calls.push(DrawCall::Point {
        position: Vec2::new(position.x, position.y),
        size,
        color: b2_hex_to_encoded(color),
    });
}

extern "C" fn draw_string_cb(
    position: sys::b2Vec2,
    text: *const std::os::raw::c_char,
    color: sys::b2HexColor,
    ctx: *mut c_void,
) {
    let calls = unsafe { &mut *(ctx as *mut Vec<DrawCall>) };
    let cstr = unsafe { std::ffi::CStr::from_ptr(text) };
    let string = cstr.to_string_lossy().into_owned();
    calls.push(DrawCall::String {
        position: Vec2::new(position.x, position.y),
        text: string,
        color: b2_hex_to_encoded(color),
    });
}

extern "C" fn draw_solid_capsule_cb(
    transform: sys::b2Transform,
    half_height: f32,
    radius: f32,
    color: sys::b2HexColor,
    ctx: *mut c_void,
) {
    let calls = unsafe { &mut *(ctx as *mut Vec<DrawCall>) };
    let center = transform.p;
    let rotation = transform.q.s.atan2(transform.q.c);
    calls.push(DrawCall::Capsule {
        center: Vec2::new(center.x, center.y),
        half_height,
        radius,
        rotation,
        color: b2_hex_to_encoded(color),
        filled: true,
    });
}

// Adapter for expected C signature: (b2Vec2, b2Vec2, f32, b2HexColor, *mut c_void)
unsafe extern "C" fn draw_solid_capsule_cb_compat(
    p1: sys::b2Vec2,
    p2: sys::b2Vec2,
    radius: f32,
    color: sys::b2HexColor,
    ctx: *mut c_void,
) {
    let calls = &mut *(ctx as *mut Vec<DrawCall>);
    // Compute center and half_height from p1 and p2
    let center = Vec2::new((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5);
    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    let half_height = ((dx * dx + dy * dy).sqrt()) * 0.5;
    let rotation = dy.atan2(dx);
    calls.push(DrawCall::Capsule {
        center,
        half_height,
        radius,
        rotation,
        color: b2_hex_to_encoded(color),
        filled: true,
    });
}

// Helper to extract rectangle from polygon (assumes 4 points, axis-aligned or rotated)
fn polygon_to_rect(verts: &[sys::b2Vec2]) -> Option<(Vec2, Vec2, f32)> {
    if verts.len() != 4 {
        return None;
    }
    let c = Vec2::new((verts[0].x + verts[2].x) * 0.5, (verts[0].y + verts[2].y) * 0.5);
    let dx = Vec2::new(verts[1].x - verts[0].x, verts[1].y - verts[0].y);
    let dy = Vec2::new(verts[3].x - verts[0].x, verts[3].y - verts[0].y);
    let half_extents = Vec2::new(dx.length() * 0.5, dy.length() * 0.5);
    let rotation = dx.y.atan2(dx.x);
    Some((c, half_extents, rotation))
}

extern "C" fn draw_polygon_cb(verts: *const sys::b2Vec2, count: i32, color: sys::b2HexColor, ctx: *mut c_void) {
    let calls = unsafe { &mut *(ctx as *mut Vec<DrawCall>) };
    let verts_slice = unsafe { std::slice::from_raw_parts(verts, count as usize) };
    if let Some((center, half_extents, rotation)) = polygon_to_rect(verts_slice) {
        calls.push(DrawCall::Rect {
            center,
            half_extents,
            rotation,
            color: b2_hex_to_encoded(color),
            filled: false,
        });
    }
}

unsafe extern "C" fn draw_solid_polygon_cb(
    _transform: sys::b2Transform,
    verts: *const sys::b2Vec2,
    count: i32,
    _radius: f32,
    color: sys::b2HexColor,
    ctx: *mut c_void,
) {
    let calls = &mut *(ctx as *mut Vec<DrawCall>);
    let verts_slice = std::slice::from_raw_parts(verts, count as usize);
    if let Some((center, half_extents, rotation)) = polygon_to_rect(verts_slice) {
        calls.push(DrawCall::Rect {
            center,
            half_extents,
            rotation,
            color: b2_hex_to_encoded(color),
            filled: true,
        });
    }
}
