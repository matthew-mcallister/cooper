pub fn mat4(mat3: na::Matrix3<f32>, w: na::Vector3<f32>) -> na::Matrix4<f32> {
    [
        [mat3.m11, mat3.m21, mat3.m31, w.x],
        [mat3.m12, mat3.m22, mat3.m32, w.y],
        [mat3.m13, mat3.m23, mat3.m33, w.z],
        [0.0,      0.0,      0.0,      1.0],
    ].into()
}

pub fn vec4(vec3: na::Vector3<f32>, w: f32) -> na::Vector4<f32> {
    [vec3.x, vec3.y, vec3.z, w].into()
}

// A rigid transform + scaling that keeps components separate.
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub rot: na::Matrix3<f32>,
    pub pos: na::Vector3<f32>,
    pub scale: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Transform::identity()
    }
}

impl std::ops::Mul for Transform {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Transform {
            rot: self.rot * rhs.rot,
            pos: self.scale * self.rot * rhs.pos + self.pos,
            scale: self.scale * rhs.scale
        }
    }
}

impl Transform {
    pub fn to_matrix(self) -> na::Matrix4<f32> {
        mat4(self.scale * self.rot, self.pos)
    }

    pub fn identity() -> Self {
        Transform {
            rot: na::one(),
            pos: na::zero(),
            scale: 1.0,
        }
    }

    pub fn inverse(self) -> Self {
        let inv_rot = self.rot.transpose();
        let inv_scale = 1.0 / self.scale;
        Transform {
            rot: inv_rot,
            pos: -inv_scale * (inv_rot * self.pos),
            scale: inv_scale,
        }
    }
}

impl Into<na::Matrix4<f32>> for Transform {
    fn into(self) -> na::Matrix4<f32> {
        self.to_matrix()
    }
}

/// Defines a right-handed perspective transform for a frustum along.
/// By this definition, (0, 0) is the middle of the screen, +x goes to
/// the right of the screen, +y goes to the bottom of the screen, and +z
/// goes into the screen. Furthermore, x ranges from -1 to 1, y from -1
/// to 1, and z from `min_depth` to `max_depth`.
///
/// For optimal depth precision, set `min_depth` to 1, `max_depth` to 0
/// (so `max_depth` < `min_depth`), and use a floating point depth
/// buffer. The benefits of this setup are [well known][0].
///
/// [0]: https://developer.nvidia.com/content/depth-precision-visualized
#[derive(Clone, Copy, Debug)]
pub struct PerspectiveTransform {
    pub z_near: f32,
    pub z_far: f32,
    pub aspect: f32,
    /// Tangent of half the vertical field-of-view angle
    pub tan_fovy2: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

impl PerspectiveTransform {
    pub fn to_matrix(&self) -> na::Matrix4<f32> {
        let (z_n, z_f) = (self.z_near, self.z_far);
        let (d_n, d_f) = (self.min_depth, self.max_depth);
        let tan_fovx2 = self.aspect * self.tan_fovy2;
        let (s_x, s_y) = (tan_fovx2, self.tan_fovy2);
        let c = z_f * (d_f - d_n) / (z_f - z_n);

        [
            [1.0 / s_x, 0.0,       0.0,      0.0],
            [0.0,       1.0 / s_y, 0.0,      0.0],
            [0.0,       0.0,       c + d_n,  1.0],
            [0.0,       0.0,       -z_n * c, 0.0],
        ].into()
    }
}

/// Constructs an orthonormal basis with a distinguished vector that
/// points in the target direction. The other basis vectors is fixed
/// using a preferred "up" axis, which serves the role of zenith in the
/// context of pilots' angles.
///
/// Returns the basis in the order `[forward, above, right]`, with
/// `forward` pointing from `pos` to `target`, `above` fixed by `up`,
/// and `right` orthogonal to the others.
///
/// See also `look_at`, `face_toward`.
///
/// TODO: Breaks down if `dir` and `up` are linearly dependent.
pub fn aiming_basis(
    dir: na::Vector3<f32>,
    up: na::Vector3<f32>,
) -> [na::Vector3<f32>; 3] {
    let fwd = dir.normalize();
    let above = (up - up.dot(&fwd) * fwd).normalize();
    let right = fwd.cross(&above);
    [fwd, above, right]
}

#[cfg(test)]
mod xform_tests {
    use approx::assert_relative_eq;
    use super::*;

    #[test]
    fn perspective_smoke() {
        let fovy = 90f32;
        let tan_fovy2 = (fovy / 2.0).tan();
        let (z_near, z_far) = (10e-3, 10e3);
        let aspect = 16.0 / 9.0;
        let perspective = PerspectiveTransform {
            z_near,
            z_far,
            aspect,
            tan_fovy2,
            min_depth: 1.0,
            max_depth: 0.0,
        };
        let mat = perspective.to_matrix();

        let perspective = |x: na::Vector3<f32>| {
            let x = na::Vector4::new(x.x, x.y, x.z, 1.0);
            let x = mat * x;
            (x / x.w).xyz()
        };

        let x = perspective([0.0, 0.0, z_near].into());
        assert_eq!(x, [0.0, 0.0, 1.0].into());
        let x = perspective([0.0, 0.0, z_far].into());
        assert_relative_eq!(x, [0.0, 0.0, 0.0].into());
        let x = perspective([0.0, z_near * tan_fovy2, z_near].into());
        assert_relative_eq!(x, [0.0, 1.0, 1.0].into());
        let x = perspective([z_far * tan_fovy2 * aspect, 0.0, z_far].into());
        assert_relative_eq!(x, [1.0, 0.0, 0.0].into());
        let x = perspective([
            -z_near * tan_fovy2 * aspect, -z_near * tan_fovy2, z_near,
        ].into());
        assert_relative_eq!(x, [-1.0, -1.0, 1.0].into());
    }

    #[test]
    fn aiming_smoke() {
        let o = na::Vector3::new(0.0f32, 0.0, 0.0);
        let x = na::Vector3::new(1.0f32, 0.0, 0.0);
        let y = na::Vector3::new(0.0, 1.0f32, 0.0);
        let z = na::Vector3::new(0.0, 0.0, 1.0f32);

        let [fwd, abv, rgt] = aiming_basis(x, z);
        assert_relative_eq!(fwd, x);
        assert_relative_eq!(rgt, -y);
        assert_relative_eq!(abv, z);

        let [fwd, abv, rgt] = aiming_basis(z, x);
        assert_relative_eq!(fwd, z);
        assert_relative_eq!(rgt, y);
        assert_relative_eq!(abv, x);

        let tgt = x + y + z;
        let [fwd, _, rgt] = aiming_basis(tgt - o, z);
        assert_relative_eq!(fwd * 3f32.sqrt(), tgt);
        assert_relative_eq!(rgt, (x - y).normalize());
    }
}

