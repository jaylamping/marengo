//! Exhaustive pinned URDF projection for the exact-value journal codec.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Model {
    name: String,
    links: Vec<Link>,
    joints: Vec<Joint>,
    materials: Vec<Material>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Pose {
    xyz: [f64; 3],
    rpy: [f64; 3],
}
#[derive(Debug, Serialize, Deserialize)]
struct Inertia {
    ixx: f64,
    ixy: f64,
    ixz: f64,
    iyy: f64,
    iyz: f64,
    izz: f64,
}
#[derive(Debug, Serialize, Deserialize)]
struct Inertial {
    origin: Pose,
    mass: f64,
    inertia: Inertia,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Geometry {
    Box {
        size: [f64; 3],
    },
    Cylinder {
        radius: f64,
        length: f64,
    },
    Capsule {
        radius: f64,
        length: f64,
    },
    Sphere {
        radius: f64,
    },
    Mesh {
        filename: String,
        scale: Option<[f64; 3]>,
    },
}
#[derive(Debug, Serialize, Deserialize)]
struct Material {
    name: String,
    color: Option<[f64; 4]>,
    texture: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Visual {
    name: Option<String>,
    origin: Pose,
    geometry: Geometry,
    material: Option<Material>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Collision {
    name: Option<String>,
    origin: Pose,
    geometry: Geometry,
}
#[derive(Debug, Serialize, Deserialize)]
struct Link {
    name: String,
    inertial: Inertial,
    visual: Vec<Visual>,
    collision: Vec<Collision>,
}
#[derive(Debug, Serialize, Deserialize)]
enum JointType {
    Revolute,
    Continuous,
    Prismatic,
    Fixed,
    Floating,
    Planar,
    Spherical,
}
#[derive(Debug, Serialize, Deserialize)]
struct Limit {
    lower: f64,
    upper: f64,
    effort: f64,
    velocity: f64,
}
#[derive(Debug, Serialize, Deserialize)]
struct Dynamics {
    damping: f64,
    friction: f64,
}
#[derive(Debug, Serialize, Deserialize)]
struct Mimic {
    joint: String,
    multiplier: Option<f64>,
    offset: Option<f64>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Safety {
    soft_lower_limit: f64,
    soft_upper_limit: f64,
    k_position: f64,
    k_velocity: f64,
}
#[derive(Debug, Serialize, Deserialize)]
struct Joint {
    name: String,
    joint_type: JointType,
    origin: Pose,
    parent: String,
    child: String,
    axis: [f64; 3],
    limit: Limit,
    dynamics: Option<Dynamics>,
    mimic: Option<Mimic>,
    safety_controller: Option<Safety>,
}

impl From<&urdf_rs::Pose> for Pose {
    fn from(value: &urdf_rs::Pose) -> Self {
        let urdf_rs::Pose { xyz, rpy } = value;
        Self {
            xyz: xyz.0,
            rpy: rpy.0,
        }
    }
}
impl From<Pose> for urdf_rs::Pose {
    fn from(value: Pose) -> Self {
        Self {
            xyz: urdf_rs::Vec3(value.xyz),
            rpy: urdf_rs::Vec3(value.rpy),
        }
    }
}
impl From<&urdf_rs::Geometry> for Geometry {
    fn from(value: &urdf_rs::Geometry) -> Self {
        match value {
            urdf_rs::Geometry::Box { size } => Self::Box { size: size.0 },
            urdf_rs::Geometry::Cylinder { radius, length } => Self::Cylinder {
                radius: *radius,
                length: *length,
            },
            urdf_rs::Geometry::Capsule { radius, length } => Self::Capsule {
                radius: *radius,
                length: *length,
            },
            urdf_rs::Geometry::Sphere { radius } => Self::Sphere { radius: *radius },
            urdf_rs::Geometry::Mesh { filename, scale } => Self::Mesh {
                filename: filename.clone(),
                scale: scale.map(|v| v.0),
            },
        }
    }
}
impl From<Geometry> for urdf_rs::Geometry {
    fn from(value: Geometry) -> Self {
        match value {
            Geometry::Box { size } => Self::Box {
                size: urdf_rs::Vec3(size),
            },
            Geometry::Cylinder { radius, length } => Self::Cylinder { radius, length },
            Geometry::Capsule { radius, length } => Self::Capsule { radius, length },
            Geometry::Sphere { radius } => Self::Sphere { radius },
            Geometry::Mesh { filename, scale } => Self::Mesh {
                filename,
                scale: scale.map(urdf_rs::Vec3),
            },
        }
    }
}
impl From<&urdf_rs::Material> for Material {
    fn from(value: &urdf_rs::Material) -> Self {
        let urdf_rs::Material {
            name,
            color,
            texture,
        } = value;
        Self {
            name: name.clone(),
            color: color.as_ref().map(|color| {
                let urdf_rs::Color { rgba } = color;
                rgba.0
            }),
            texture: texture.as_ref().map(|texture| {
                let urdf_rs::Texture { filename } = texture;
                filename.clone()
            }),
        }
    }
}
impl From<Material> for urdf_rs::Material {
    fn from(value: Material) -> Self {
        Self {
            name: value.name,
            color: value.color.map(|rgba| urdf_rs::Color {
                rgba: urdf_rs::Vec4(rgba),
            }),
            texture: value.texture.map(|filename| urdf_rs::Texture { filename }),
        }
    }
}
impl From<&urdf_rs::Visual> for Visual {
    fn from(value: &urdf_rs::Visual) -> Self {
        let urdf_rs::Visual {
            name,
            origin,
            geometry,
            material,
        } = value;
        Self {
            name: name.clone(),
            origin: origin.into(),
            geometry: geometry.into(),
            material: material.as_ref().map(Material::from),
        }
    }
}
impl From<Visual> for urdf_rs::Visual {
    fn from(value: Visual) -> Self {
        Self {
            name: value.name,
            origin: value.origin.into(),
            geometry: value.geometry.into(),
            material: value.material.map(urdf_rs::Material::from),
        }
    }
}
impl From<&urdf_rs::Collision> for Collision {
    fn from(value: &urdf_rs::Collision) -> Self {
        let urdf_rs::Collision {
            name,
            origin,
            geometry,
        } = value;
        Self {
            name: name.clone(),
            origin: origin.into(),
            geometry: geometry.into(),
        }
    }
}
impl From<Collision> for urdf_rs::Collision {
    fn from(value: Collision) -> Self {
        Self {
            name: value.name,
            origin: value.origin.into(),
            geometry: value.geometry.into(),
        }
    }
}
impl From<&urdf_rs::Inertial> for Inertial {
    fn from(value: &urdf_rs::Inertial) -> Self {
        let urdf_rs::Inertial {
            origin,
            mass,
            inertia,
        } = value;
        let urdf_rs::Mass { value: mass } = mass;
        let urdf_rs::Inertia {
            ixx,
            ixy,
            ixz,
            iyy,
            iyz,
            izz,
        } = inertia;
        Self {
            origin: origin.into(),
            mass: *mass,
            inertia: Inertia {
                ixx: *ixx,
                ixy: *ixy,
                ixz: *ixz,
                iyy: *iyy,
                iyz: *iyz,
                izz: *izz,
            },
        }
    }
}
impl From<Inertial> for urdf_rs::Inertial {
    fn from(value: Inertial) -> Self {
        Self {
            origin: value.origin.into(),
            mass: urdf_rs::Mass { value: value.mass },
            inertia: urdf_rs::Inertia {
                ixx: value.inertia.ixx,
                ixy: value.inertia.ixy,
                ixz: value.inertia.ixz,
                iyy: value.inertia.iyy,
                iyz: value.inertia.iyz,
                izz: value.inertia.izz,
            },
        }
    }
}
impl From<&urdf_rs::Link> for Link {
    fn from(value: &urdf_rs::Link) -> Self {
        let urdf_rs::Link {
            name,
            inertial,
            visual,
            collision,
        } = value;
        Self {
            name: name.clone(),
            inertial: inertial.into(),
            visual: visual.iter().map(Visual::from).collect(),
            collision: collision.iter().map(Collision::from).collect(),
        }
    }
}
impl From<Link> for urdf_rs::Link {
    fn from(value: Link) -> Self {
        Self {
            name: value.name,
            inertial: value.inertial.into(),
            visual: value
                .visual
                .into_iter()
                .map(urdf_rs::Visual::from)
                .collect(),
            collision: value
                .collision
                .into_iter()
                .map(urdf_rs::Collision::from)
                .collect(),
        }
    }
}
impl From<&urdf_rs::JointType> for JointType {
    fn from(value: &urdf_rs::JointType) -> Self {
        match value {
            urdf_rs::JointType::Revolute => Self::Revolute,
            urdf_rs::JointType::Continuous => Self::Continuous,
            urdf_rs::JointType::Prismatic => Self::Prismatic,
            urdf_rs::JointType::Fixed => Self::Fixed,
            urdf_rs::JointType::Floating => Self::Floating,
            urdf_rs::JointType::Planar => Self::Planar,
            urdf_rs::JointType::Spherical => Self::Spherical,
        }
    }
}
impl From<JointType> for urdf_rs::JointType {
    fn from(value: JointType) -> Self {
        match value {
            JointType::Revolute => Self::Revolute,
            JointType::Continuous => Self::Continuous,
            JointType::Prismatic => Self::Prismatic,
            JointType::Fixed => Self::Fixed,
            JointType::Floating => Self::Floating,
            JointType::Planar => Self::Planar,
            JointType::Spherical => Self::Spherical,
        }
    }
}
impl From<&urdf_rs::Joint> for Joint {
    fn from(value: &urdf_rs::Joint) -> Self {
        let urdf_rs::Joint {
            name,
            joint_type,
            origin,
            parent,
            child,
            axis,
            limit,
            dynamics,
            mimic,
            safety_controller,
        } = value;
        let urdf_rs::LinkName { link: parent } = parent;
        let urdf_rs::LinkName { link: child } = child;
        let urdf_rs::Axis { xyz: axis } = axis;
        let urdf_rs::JointLimit {
            lower,
            upper,
            effort,
            velocity,
        } = limit;
        Self {
            name: name.clone(),
            joint_type: joint_type.into(),
            origin: origin.into(),
            parent: parent.clone(),
            child: child.clone(),
            axis: axis.0,
            limit: Limit {
                lower: *lower,
                upper: *upper,
                effort: *effort,
                velocity: *velocity,
            },
            dynamics: dynamics.as_ref().map(|value| {
                let urdf_rs::Dynamics { damping, friction } = value;
                Dynamics {
                    damping: *damping,
                    friction: *friction,
                }
            }),
            mimic: mimic.as_ref().map(|value| {
                let urdf_rs::Mimic {
                    joint,
                    multiplier,
                    offset,
                } = value;
                Mimic {
                    joint: joint.clone(),
                    multiplier: *multiplier,
                    offset: *offset,
                }
            }),
            safety_controller: safety_controller.as_ref().map(|value| {
                let urdf_rs::SafetyController {
                    soft_lower_limit,
                    soft_upper_limit,
                    k_position,
                    k_velocity,
                } = value;
                Safety {
                    soft_lower_limit: *soft_lower_limit,
                    soft_upper_limit: *soft_upper_limit,
                    k_position: *k_position,
                    k_velocity: *k_velocity,
                }
            }),
        }
    }
}
impl From<Joint> for urdf_rs::Joint {
    fn from(value: Joint) -> Self {
        Self {
            name: value.name,
            joint_type: value.joint_type.into(),
            origin: value.origin.into(),
            parent: urdf_rs::LinkName { link: value.parent },
            child: urdf_rs::LinkName { link: value.child },
            axis: urdf_rs::Axis {
                xyz: urdf_rs::Vec3(value.axis),
            },
            limit: urdf_rs::JointLimit {
                lower: value.limit.lower,
                upper: value.limit.upper,
                effort: value.limit.effort,
                velocity: value.limit.velocity,
            },
            dynamics: value.dynamics.map(|value| urdf_rs::Dynamics {
                damping: value.damping,
                friction: value.friction,
            }),
            mimic: value.mimic.map(|value| urdf_rs::Mimic {
                joint: value.joint,
                multiplier: value.multiplier,
                offset: value.offset,
            }),
            safety_controller: value
                .safety_controller
                .map(|value| urdf_rs::SafetyController {
                    soft_lower_limit: value.soft_lower_limit,
                    soft_upper_limit: value.soft_upper_limit,
                    k_position: value.k_position,
                    k_velocity: value.k_velocity,
                }),
        }
    }
}
impl From<&urdf_rs::Robot> for Model {
    fn from(value: &urdf_rs::Robot) -> Self {
        let urdf_rs::Robot {
            name,
            links,
            joints,
            materials,
        } = value;
        Self {
            name: name.clone(),
            links: links.iter().map(Link::from).collect(),
            joints: joints.iter().map(Joint::from).collect(),
            materials: materials.iter().map(Material::from).collect(),
        }
    }
}
impl From<Model> for urdf_rs::Robot {
    fn from(value: Model) -> Self {
        Self {
            name: value.name,
            links: value.links.into_iter().map(urdf_rs::Link::from).collect(),
            joints: value.joints.into_iter().map(urdf_rs::Joint::from).collect(),
            materials: value
                .materials
                .into_iter()
                .map(urdf_rs::Material::from)
                .collect(),
        }
    }
}
