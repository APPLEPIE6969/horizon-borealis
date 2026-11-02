//! Small column-major mat4 helpers for the harness's cameras, matching the
//! WGSL `mat4x4<f32>` layout (each `[f32; 4]` a column) and Horizon's frame:
//! reversed depth in wgpu's clip space (z 0..1, 1 near), and the shadow
//! camera's orthographic clip (z 0 at the light, 1 away).

/// Column-major: `m[col][row]`.
pub type Mat4 = [[f32; 4]; 4];

pub const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            out[col][row] = (0..4).map(|k| a[k][row] * b[col][k]).sum();
        }
    }
    out
}

pub fn mul_vec(m: &Mat4, v: [f32; 4]) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    for row in 0..4 {
        out[row] = (0..4).map(|k| m[k][row] * v[k]).sum();
    }
    out
}

/// The 3x3 part's transpose-inverse is the inverse for rotations; this is a
/// general inverse via Gauss-Jordan, fine at these sizes.
pub fn invert(m: &Mat4) -> Mat4 {
    let mut a = *m;
    let mut inv = IDENTITY;
    for col in 0..4 {
        // Find the pivot row.
        let mut pivot = col;
        for row in col..4 {
            if a[col][row].abs() > a[col][pivot].abs() {
                pivot = row;
            }
        }
        if a[col][pivot].abs() < 1e-12 {
            return IDENTITY;
        }
        a.swap(col, pivot);
        inv.swap(col, pivot);
        let d = a[col][col];
        for row in 0..4 {
            a[col][row] /= d;
            inv[col][row] /= d;
        }
        for other in 0..4 {
            if other == col {
                continue;
            }
            let f = a[other][col];
            for row in 0..4 {
                a[other][row] -= a[col][row] * f;
                inv[other][row] -= inv[col][row] * f;
            }
        }
    }
    inv
}

/// A view matrix: the camera's rotation only (Horizon's `frame.view` maps
/// camera-relative world space to view space, no translation).
pub fn view(eye: [f32; 3], target: [f32; 3]) -> Mat4 {
    let up = [0.0f32, 1.0, 0.0];
    let mut f = sub(target, eye);
    let fl = len(f);
    if fl < 1e-6 {
        f = [0.0, 0.0, -1.0];
    } else {
        f = [f[0] / fl, f[1] / fl, f[2] / fl];
    }
    let mut s = cross(f, up);
    let sl = len(s);
    if sl < 1e-6 {
        s = [1.0, 0.0, 0.0];
    } else {
        s = [s[0] / sl, s[1] / sl, s[2] / sl];
    }
    let u = cross(s, f);
    // View space looks down -z: the rows are the camera's axes.
    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// Perspective projection, reversed depth (1 at the near plane, 0 at `far`),
/// wgpu's clip space (z 0..1). Vertical `fov_y` in radians.
pub fn perspective_reversed(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fov_y * 0.5).tan();
    // z_ndc = (A*z_view + B) / (-z_view): 1 at -near, 0 at -far.
    let a = near / (far - near);
    let b = far * near / (far - near);
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, a, -1.0],
        [0.0, 0.0, b, 0.0],
    ]
}

/// An orthographic projection for the shadow map: the box [-half, half] on x
/// and y, and z over `depth` from the light (clip z 0 at the light).
pub fn orthographic_shadow(half: f32, depth: f32) -> Mat4 {
    [
        [1.0 / half, 0.0, 0.0, 0.0],
        [0.0, 1.0 / half, 0.0, 0.0],
        [0.0, 0.0, 1.0 / depth, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// The shadow camera's view: looking along `-light` from `lift` blocks back,
/// y up where that is possible. Positions are camera-relative, so the scene
/// sits around the origin and the light's offset only shifts z.
///
/// The z row is the direction from the light toward the scene: a point's
/// clip z grows with its distance from the light (0 at it, per the frame's
/// `shadow_view_proj` contract), so nearer occluders win the depth test.
pub fn shadow_view(light: [f32; 3], lift: f32) -> Mat4 {
    let f = normalize(neg(light)); // from the light toward the scene
    let mut up = [0.0f32, 1.0, 0.0];
    if light[1].abs() > 0.99 {
        up = [0.0, 0.0, 1.0];
    }
    let s = normalize(cross(f, up));
    let u = cross(s, f);
    let lift_z = lift; // the origin sits `lift` blocks from the light
    [
        [s[0], u[0], f[0], 0.0],
        [s[1], u[1], f[1], 0.0],
        [s[2], u[2], f[2], 0.0],
        [0.0, 0.0, lift_z, 1.0],
    ]
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn len(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

pub fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = len(a);
    if l < 1e-8 {
        [0.0, 1.0, 0.0]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

pub fn neg(a: [f32; 3]) -> [f32; 3] {
    [-a[0], -a[1], -a[2]]
}
