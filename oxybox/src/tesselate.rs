use bitflags::bitflags;
use glam::Vec2;
use lyon::math::Point;
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator,
    StrokeVertex, VertexBuffers,
};
use oxybox_sys as sys;
use smol_rgb::EncodedColor;
use std::ffi::c_void;

use crate::{World, WorldId};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: EncodedColor,
}

#[derive(Default, Debug)]
pub struct RenderMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

bitflags! {
    pub struct TesselationFlags: u32 {
        const SHAPES             = 0b0001;
        const JOINTS             = 0b0010;
        const JOINT_EXTRAS       = 0b0100;
        const BOUNDS             = 0b1000;
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

// --- tiny, explicit knobs (kept local so you can see/tweak them) ---
const CIRCLE_SEGMENTS: usize = 32;
const TOLERANCE: f32 = 0.02;      // world units
const STROKE_WIDTH: f32 = 0.02;   // world units

// free fn; feel free to replace with `impl From<sys::b2HexColor> for EncodedColor` or similar
#[inline]
fn b2hex_to_encoded(c: sys::b2HexColor) -> EncodedColor {
    let r = ((c >> 16) & 0xFF) as u8;
    let g = ((c >> 8) & 0xFF) as u8;
    let b = (c & 0xFF) as u8;
    EncodedColor::new(r, g, b, 255)
}

// ---------------- public entrypoint ----------------

pub fn tesselate(world: World, flags: TesselationFlags) -> RenderMesh {
    let mut ctx = LyonCtx::new();

    let mut dd = sys::b2DebugDraw {
        // supported: polygons + circles
        DrawCircleFcn: Some(draw_circle_cb),
        DrawSolidCircleFcn: Some(draw_solid_circle_cb),
        DrawPolygonFcn: Some(draw_polygon_cb),
        DrawSolidPolygonFcn: Some(draw_solid_polygon_cb),

        // unsupported -> None (no panics)
        DrawSolidCapsuleFcn: None,
        DrawSegmentFcn: None,
        DrawTransformFcn: None,
        DrawPointFcn: None,
        DrawStringFcn: None,

        drawShapes: flags.contains(TesselationFlags::SHAPES),
        drawJoints: flags.contains(TesselationFlags::JOINTS),
        drawJointExtras: flags.contains(TesselationFlags::JOINT_EXTRAS),
        drawBounds: flags.contains(TesselationFlags::BOUNDS),
        drawMass: flags.contains(TesselationFlags::MASS),
        drawBodyNames: flags.contains(TesselationFlags::BODY_NAMES),
        drawContacts: flags.contains(TesselationFlags::CONTACTS),
        drawGraphColors: flags.contains(TesselationFlags::GRAPH_COLORS),
        drawContactNormals: flags.contains(TesselationFlags::CONTACT_NORMALS),
        drawContactImpulses: flags.contains(TesselationFlags::CONTACT_IMPULSES),
        drawContactFeatures: flags.contains(TesselationFlags::CONTACT_FEATURES),
        drawFrictionImpulses: flags.contains(TesselationFlags::FRICTION_IMPULSES),
        drawIslands: flags.contains(TesselationFlags::ISLANDS),
        drawingBounds: sys::b2AABB {
            lowerBound: sys::b2Vec2 { x: -1.0e6, y: -1.0e6 },
            upperBound: sys::b2Vec2 { x:  1.0e6, y:  1.0e6 },
        },
        context: &mut ctx as *mut _ as *mut c_void,
    };

    unsafe { sys::b2World_Draw(*world.id(), &mut dd as *mut _) };

    ctx.into_mesh()
}

// ---------------- minimal lyon context ----------------

struct LyonCtx {
    fill_tess: FillTessellator,
    stroke_tess: StrokeTessellator,
    buffers: VertexBuffers<Vertex, u32>,
}

impl LyonCtx {
    fn new() -> Self {
        Self {
            fill_tess: FillTessellator::new(),
            stroke_tess: StrokeTessellator::new(),
            buffers: VertexBuffers::new(),
        }
    }

    fn into_mesh(self) -> RenderMesh {
        RenderMesh {
            vertices: self.buffers.vertices,
            indices: self.buffers.indices,
        }
    }

    fn push_fill(&mut self, path: &Path, color: EncodedColor) {
        let mut builder = BuffersBuilder::new(&mut self.buffers, |v: FillVertex| Vertex {
            pos: v.position().to_array(),
            color,
        });
        let opts = FillOptions::tolerance(TOLERANCE);
        let _ = self.fill_tess.tessellate_path(path, &opts, &mut builder);
    }

    fn push_stroke(&mut self, path: &Path, color: EncodedColor) {
        let mut builder = BuffersBuilder::new(&mut self.buffers, |v: StrokeVertex| Vertex {
            pos: v.position().to_array(),
            color,
        });
        let opts = StrokeOptions::tolerance(TOLERANCE).with_line_width(STROKE_WIDTH);
        let _ = self.stroke_tess.tessellate_path(path, &opts, &mut builder);
    }

    fn polygon_path(points: &[[f32; 2]]) -> Path {
        assert!(points.len() >= 2, "polygon must have at least 2 points");
        let mut builder = Path::builder();
        builder.begin(Point::new(points[0][0], points[0][1]));
        for p in &points[1..] {
            builder.line_to(Point::new(p[0], p[1]));
        }
        builder.close();
        builder.build()
    }

    fn circle_path(center: Vec2, radius: f32, segments_hint: usize) -> Path {
        let segments = segments_hint.max(12);
        let mut builder = Path::builder();
        let start = Point::new(center.x + radius, center.y);
        builder.begin(start);
        for i in 1..=segments {
            let t = (i as f32) * std::f32::consts::TAU / (segments as f32);
            let pt = Point::new(center.x + radius * t.cos(), center.y + radius * t.sin());
            builder.line_to(pt);
        }
        builder.close();
        builder.build()
    }
}

// ---------------- Box2D callbacks ----------------

extern "C" fn draw_circle_cb(center: sys::b2Vec2, radius: f32, color: sys::b2HexColor, ctx: *mut c_void) {
    let ctx = unsafe { &mut *(ctx as *mut LyonCtx) };
    let encoded = b2hex_to_encoded(color);
    let path = LyonCtx::circle_path(Vec2::new(center.x, center.y), radius, CIRCLE_SEGMENTS);
    ctx.push_stroke(&path, encoded);
}

extern "C" fn draw_solid_circle_cb(transform: sys::b2Transform, radius: f32, color: sys::b2HexColor, ctx: *mut c_void) {
    let ctx = unsafe { &mut *(ctx as *mut LyonCtx) };
    let encoded = b2hex_to_encoded(color);
    let c = transform.p;
    let path = LyonCtx::circle_path(Vec2::new(c.x, c.y), radius, CIRCLE_SEGMENTS);
    ctx.push_fill(&path, encoded);
}

extern "C" fn draw_polygon_cb(verts: *const sys::b2Vec2, count: i32, color: sys::b2HexColor, ctx: *mut c_void) {
    let ctx = unsafe { &mut *(ctx as *mut LyonCtx) };
    let encoded = b2hex_to_encoded(color);
    let raw = unsafe { std::slice::from_raw_parts(verts, count as usize) };
    assert!(raw.len() >= 2, "wire polygon must have at least 2 vertices");

    let points: Vec<[f32; 2]> = raw.iter().map(|v| [v.x, v.y]).collect();
    let path = LyonCtx::polygon_path(&points);
    ctx.push_stroke(&path, encoded);
}

unsafe extern "C" fn draw_solid_polygon_cb(
    _transform: sys::b2Transform,
    verts: *const sys::b2Vec2,
    count: i32,
    _radius: f32,
    color: sys::b2HexColor,
    ctx: *mut c_void,
) {
    let ctx = &mut *(ctx as *mut LyonCtx);
    let encoded = b2hex_to_encoded(color);
    let raw = std::slice::from_raw_parts(verts, count as usize);
    assert!(raw.len() >= 3, "filled polygon must have at least 3 vertices");

    let points: Vec<[f32; 2]> = raw.iter().map(|v| [v.x, v.y]).collect();
    let path = LyonCtx::polygon_path(&points);
    ctx.push_fill(&path, encoded);
}
