use h7cad_native_model::EntityData;

pub(crate) fn parse_line(codes: &[(i16, String)]) -> EntityData {
    let (mut sx, mut sy, mut sz) = (0.0, 0.0, 0.0);
    let (mut ex, mut ey, mut ez) = (0.0, 0.0, 0.0);
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => sx = v,
            20 => sy = v,
            30 => sz = v,
            11 => ex = v,
            21 => ey = v,
            31 => ez = v,
            _ => {}
        }
    }
    EntityData::Line {
        start: [sx, sy, sz],
        end: [ex, ey, ez],
    }
}

pub(crate) fn parse_circle(codes: &[(i16, String)]) -> EntityData {
    let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
    let mut r = 0.0;
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => cx = v,
            20 => cy = v,
            30 => cz = v,
            40 => r = v,
            _ => {}
        }
    }
    EntityData::Circle {
        center: [cx, cy, cz],
        radius: r,
    }
}

pub(crate) fn parse_arc(codes: &[(i16, String)]) -> EntityData {
    let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
    let mut r = 0.0;
    let (mut sa, mut ea) = (0.0, 360.0);
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => cx = v,
            20 => cy = v,
            30 => cz = v,
            40 => r = v,
            50 => sa = v,
            51 => ea = v,
            _ => {}
        }
    }
    EntityData::Arc {
        center: [cx, cy, cz],
        radius: r,
        start_angle: sa,
        end_angle: ea,
    }
}

pub(crate) fn parse_point(codes: &[(i16, String)]) -> EntityData {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => x = v,
            20 => y = v,
            30 => z = v,
            _ => {}
        }
    }
    EntityData::Point {
        position: [x, y, z],
    }
}

pub(crate) fn parse_lwpolyline(codes: &[(i16, String)]) -> EntityData {
    use h7cad_native_model::LwVertex;

    let mut vertices: Vec<LwVertex> = Vec::new();
    let mut closed = false;
    let mut cur_x = 0.0;
    let mut cur_y = 0.0;
    let mut cur_bulge = 0.0;
    let mut has_vertex = false;

    for &(code, ref val) in codes {
        match code {
            70 => closed = val.parse::<i16>().unwrap_or(0) & 1 != 0,
            10 => {
                if has_vertex {
                    vertices.push(LwVertex {
                        x: cur_x,
                        y: cur_y,
                        bulge: cur_bulge,
                    });
                    cur_bulge = 0.0;
                }
                cur_x = val.parse().unwrap_or(0.0);
                cur_y = 0.0;
                has_vertex = true;
            }
            20 => cur_y = val.parse().unwrap_or(0.0),
            42 => cur_bulge = val.parse().unwrap_or(0.0),
            _ => {}
        }
    }
    if has_vertex {
        vertices.push(LwVertex {
            x: cur_x,
            y: cur_y,
            bulge: cur_bulge,
        });
    }

    EntityData::LwPolyline { vertices, closed }
}

pub(crate) fn parse_attrib(codes: &[(i16, String)]) -> EntityData {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let mut height = 0.0;
    let mut tag = String::new();
    let mut value = String::new();
    for &(code, ref val) in codes {
        match code {
            10 => x = val.parse().unwrap_or(0.0),
            20 => y = val.parse().unwrap_or(0.0),
            30 => z = val.parse().unwrap_or(0.0),
            40 => height = val.parse().unwrap_or(0.0),
            1 => value = val.clone(),
            2 => tag = val.clone(),
            _ => {}
        }
    }
    EntityData::Attrib {
        tag,
        value,
        insertion: [x, y, z],
        height,
    }
}

pub(crate) fn parse_attdef(codes: &[(i16, String)]) -> EntityData {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let mut height = 0.0;
    let mut tag = String::new();
    let mut prompt = String::new();
    let mut default_value = String::new();
    for &(code, ref val) in codes {
        match code {
            10 => x = val.parse().unwrap_or(0.0),
            20 => y = val.parse().unwrap_or(0.0),
            30 => z = val.parse().unwrap_or(0.0),
            40 => height = val.parse().unwrap_or(0.0),
            1 => default_value = val.clone(),
            2 => tag = val.clone(),
            3 => prompt = val.clone(),
            _ => {}
        }
    }
    EntityData::AttDef {
        tag,
        prompt,
        default_value,
        insertion: [x, y, z],
        height,
    }
}

pub(crate) fn parse_leader(codes: &[(i16, String)]) -> EntityData {
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut has_arrowhead = true;
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let mut has_vertex = false;
    for &(code, ref val) in codes {
        match code {
            71 => has_arrowhead = val.parse::<i16>().unwrap_or(1) != 0,
            10 => {
                if has_vertex {
                    vertices.push([x, y, z]);
                }
                x = val.parse().unwrap_or(0.0);
                y = 0.0;
                z = 0.0;
                has_vertex = true;
            }
            20 => y = val.parse().unwrap_or(0.0),
            30 => z = val.parse().unwrap_or(0.0),
            _ => {}
        }
    }
    if has_vertex {
        vertices.push([x, y, z]);
    }
    EntityData::Leader {
        vertices,
        has_arrowhead,
    }
}

pub(crate) fn parse_mline(codes: &[(i16, String)]) -> EntityData {
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut style_name = String::new();
    let mut scale = 1.0;
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let mut has_vertex = false;
    for &(code, ref val) in codes {
        match code {
            2 => style_name = val.clone(),
            40 => scale = val.parse().unwrap_or(1.0),
            11 => {
                if has_vertex {
                    vertices.push([x, y, z]);
                }
                x = val.parse().unwrap_or(0.0);
                y = 0.0;
                z = 0.0;
                has_vertex = true;
            }
            21 => y = val.parse().unwrap_or(0.0),
            31 => z = val.parse().unwrap_or(0.0),
            _ => {}
        }
    }
    if has_vertex {
        vertices.push([x, y, z]);
    }
    EntityData::MLine {
        vertices,
        style_name,
        scale,
    }
}

pub(crate) fn parse_image(codes: &[(i16, String)]) -> EntityData {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let (mut ux, mut uy, mut uz) = (1.0, 0.0, 0.0);
    let (mut vx, mut vy, mut vz) = (0.0, 1.0, 0.0);
    let (mut sx, mut sy) = (1.0, 1.0);
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => x = v,
            20 => y = v,
            30 => z = v,
            11 => ux = v,
            21 => uy = v,
            31 => uz = v,
            12 => vx = v,
            22 => vy = v,
            32 => vz = v,
            13 => sx = v,
            23 => sy = v,
            _ => {}
        }
    }
    EntityData::Image {
        insertion: [x, y, z],
        u_vector: [ux, uy, uz],
        v_vector: [vx, vy, vz],
        image_size: [sx, sy],
    }
}

pub(crate) fn parse_wipeout(codes: &[(i16, String)]) -> EntityData {
    let mut vertices: Vec<[f64; 2]> = Vec::new();
    let (mut x, mut y) = (0.0, 0.0);
    let mut has_vertex = false;
    for &(code, ref val) in codes {
        match code {
            14 => {
                if has_vertex {
                    vertices.push([x, y]);
                }
                x = val.parse().unwrap_or(0.0);
                y = 0.0;
                has_vertex = true;
            }
            24 => y = val.parse().unwrap_or(0.0),
            _ => {}
        }
    }
    if has_vertex {
        vertices.push([x, y]);
    }
    EntityData::Wipeout {
        clip_vertices: vertices,
    }
}

pub(crate) fn parse_tolerance(codes: &[(i16, String)]) -> EntityData {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let mut text = String::new();
    for &(code, ref val) in codes {
        match code {
            10 => x = val.parse().unwrap_or(0.0),
            20 => y = val.parse().unwrap_or(0.0),
            30 => z = val.parse().unwrap_or(0.0),
            1 => text = val.clone(),
            _ => {}
        }
    }
    EntityData::Tolerance {
        text,
        insertion: [x, y, z],
    }
}

pub(crate) fn parse_shape(codes: &[(i16, String)]) -> EntityData {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let mut size = 0.0;
    let mut shape_number: i16 = 0;
    for &(code, ref val) in codes {
        match code {
            10 => x = val.parse().unwrap_or(0.0),
            20 => y = val.parse().unwrap_or(0.0),
            30 => z = val.parse().unwrap_or(0.0),
            40 => size = val.parse().unwrap_or(0.0),
            2 => shape_number = val.parse().unwrap_or(0),
            _ => {}
        }
    }
    EntityData::Shape {
        insertion: [x, y, z],
        size,
        shape_number,
    }
}

pub(crate) fn parse_solid3d(codes: &[(i16, String)]) -> EntityData {
    let mut acis_data = String::new();
    for &(code, ref val) in codes {
        if code == 1 || code == 3 {
            acis_data.push_str(val);
            acis_data.push('\n');
        }
    }
    EntityData::Solid3D { acis_data }
}

pub(crate) fn parse_region(codes: &[(i16, String)]) -> EntityData {
    let mut acis_data = String::new();
    for &(code, ref val) in codes {
        if code == 1 || code == 3 {
            acis_data.push_str(val);
            acis_data.push('\n');
        }
    }
    EntityData::Region { acis_data }
}

pub(crate) fn parse_mesh(codes: &[(i16, String)]) -> EntityData {
    let mut vertex_count: i32 = 0;
    let mut face_count: i32 = 0;
    for &(code, ref val) in codes {
        match code {
            92 => vertex_count = val.parse().unwrap_or(0),
            93 => face_count = val.parse().unwrap_or(0),
            _ => {}
        }
    }
    EntityData::Mesh {
        vertex_count,
        face_count,
    }
}

pub(crate) fn parse_underlay(codes: &[(i16, String)]) -> EntityData {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let (mut sx, mut sy, mut sz) = (1.0, 1.0, 1.0);
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => x = v,
            20 => y = v,
            30 => z = v,
            41 => sx = v,
            42 => sy = v,
            43 => sz = v,
            _ => {}
        }
    }
    EntityData::PdfUnderlay {
        insertion: [x, y, z],
        scale: [sx, sy, sz],
    }
}

pub(crate) fn parse_ellipse(codes: &[(i16, String)]) -> EntityData {
    let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
    let (mut mx, mut my, mut mz) = (1.0, 0.0, 0.0);
    let mut ratio = 1.0;
    let (mut sp, mut ep) = (0.0, std::f64::consts::TAU);
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => cx = v,
            20 => cy = v,
            30 => cz = v,
            11 => mx = v,
            21 => my = v,
            31 => mz = v,
            40 => ratio = v,
            41 => sp = v,
            42 => ep = v,
            _ => {}
        }
    }
    EntityData::Ellipse {
        center: [cx, cy, cz],
        major_axis: [mx, my, mz],
        ratio,
        start_param: sp,
        end_param: ep,
    }
}

pub(crate) fn parse_spline(codes: &[(i16, String)]) -> EntityData {
    let mut degree: i32 = 3;
    let mut closed = false;
    let mut knots: Vec<f64> = Vec::new();
    let mut control_points: Vec<[f64; 3]> = Vec::new();
    let mut fit_points: Vec<[f64; 3]> = Vec::new();
    let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
    let (mut fx, mut fy, mut fz) = (0.0, 0.0, 0.0);
    let mut in_control = false;
    let mut in_fit = false;

    for &(code, ref val) in codes {
        match code {
            71 => degree = val.parse().unwrap_or(3),
            70 => closed = val.parse::<i16>().unwrap_or(0) & 1 != 0,
            40 => knots.push(val.parse().unwrap_or(0.0)),
            10 => {
                if in_control {
                    control_points.push([cx, cy, cz]);
                }
                cx = val.parse().unwrap_or(0.0);
                cy = 0.0;
                cz = 0.0;
                in_control = true;
                in_fit = false;
            }
            20 => {
                if in_control {
                    cy = val.parse().unwrap_or(0.0);
                } else if in_fit {
                    fy = val.parse().unwrap_or(0.0);
                }
            }
            30 => {
                if in_control {
                    cz = val.parse().unwrap_or(0.0);
                } else if in_fit {
                    fz = val.parse().unwrap_or(0.0);
                }
            }
            11 => {
                if in_fit {
                    fit_points.push([fx, fy, fz]);
                }
                if in_control {
                    control_points.push([cx, cy, cz]);
                    in_control = false;
                }
                fx = val.parse().unwrap_or(0.0);
                fy = 0.0;
                fz = 0.0;
                in_fit = true;
            }
            21 => {
                if in_fit {
                    fy = val.parse().unwrap_or(0.0);
                }
            }
            31 => {
                if in_fit {
                    fz = val.parse().unwrap_or(0.0);
                }
            }
            _ => {}
        }
    }
    if in_control {
        control_points.push([cx, cy, cz]);
    }
    if in_fit {
        fit_points.push([fx, fy, fz]);
    }

    EntityData::Spline {
        degree,
        closed,
        knots,
        control_points,
        fit_points,
    }
}

pub(crate) fn parse_3dface(codes: &[(i16, String)]) -> EntityData {
    let mut corners = [[0.0; 3]; 4];
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => corners[0][0] = v,
            20 => corners[0][1] = v,
            30 => corners[0][2] = v,
            11 => corners[1][0] = v,
            21 => corners[1][1] = v,
            31 => corners[1][2] = v,
            12 => corners[2][0] = v,
            22 => corners[2][1] = v,
            32 => corners[2][2] = v,
            13 => corners[3][0] = v,
            23 => corners[3][1] = v,
            33 => corners[3][2] = v,
            _ => {}
        }
    }
    EntityData::Face3D { corners }
}

pub(crate) fn parse_solid(codes: &[(i16, String)]) -> EntityData {
    let mut corners = [[0.0; 3]; 4];
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => corners[0][0] = v,
            20 => corners[0][1] = v,
            30 => corners[0][2] = v,
            11 => corners[1][0] = v,
            21 => corners[1][1] = v,
            31 => corners[1][2] = v,
            12 => corners[2][0] = v,
            22 => corners[2][1] = v,
            32 => corners[2][2] = v,
            13 => corners[3][0] = v,
            23 => corners[3][1] = v,
            33 => corners[3][2] = v,
            _ => {}
        }
    }
    EntityData::Solid { corners }
}

pub(crate) fn parse_ray_xline(codes: &[(i16, String)], is_ray: bool) -> EntityData {
    let (mut ox, mut oy, mut oz) = (0.0, 0.0, 0.0);
    let (mut dx, mut dy, mut dz) = (1.0, 0.0, 0.0);
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => ox = v,
            20 => oy = v,
            30 => oz = v,
            11 => dx = v,
            21 => dy = v,
            31 => dz = v,
            _ => {}
        }
    }
    if is_ray {
        EntityData::Ray {
            origin: [ox, oy, oz],
            direction: [dx, dy, dz],
        }
    } else {
        EntityData::XLine {
            origin: [ox, oy, oz],
            direction: [dx, dy, dz],
        }
    }
}

pub(crate) fn parse_mtext(codes: &[(i16, String)]) -> EntityData {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let mut height = 0.0;
    let mut width = 0.0;
    let mut rotation = 0.0;
    let mut value = String::new();
    for &(code, ref val) in codes {
        match code {
            10 => x = val.parse().unwrap_or(0.0),
            20 => y = val.parse().unwrap_or(0.0),
            30 => z = val.parse().unwrap_or(0.0),
            40 => height = val.parse().unwrap_or(0.0),
            41 => width = val.parse().unwrap_or(0.0),
            50 => rotation = val.parse().unwrap_or(0.0),
            1 | 3 => value.push_str(val),
            _ => {}
        }
    }
    EntityData::MText {
        insertion: [x, y, z],
        height,
        width,
        value,
        rotation,
    }
}

pub(crate) fn parse_insert(codes: &[(i16, String)]) -> (EntityData, bool) {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let (mut sx, mut sy, mut sz) = (1.0, 1.0, 1.0);
    let mut rotation = 0.0;
    let mut block_name = String::new();
    let mut has_attribs = false;
    for &(code, ref val) in codes {
        match code {
            2 => block_name = val.clone(),
            10 => x = val.parse().unwrap_or(0.0),
            20 => y = val.parse().unwrap_or(0.0),
            30 => z = val.parse().unwrap_or(0.0),
            41 => sx = val.parse().unwrap_or(1.0),
            42 => sy = val.parse().unwrap_or(1.0),
            43 => sz = val.parse().unwrap_or(1.0),
            50 => rotation = val.parse().unwrap_or(0.0),
            66 => has_attribs = val.parse::<i16>().unwrap_or(0) == 1,
            _ => {}
        }
    }
    (EntityData::Insert {
        block_name,
        insertion: [x, y, z],
        scale: [sx, sy, sz],
        rotation,
        has_attribs,
        attribs: Vec::new(),
    }, has_attribs)
}

pub(crate) fn parse_dimension(codes: &[(i16, String)]) -> EntityData {
    let mut dim_type: i16 = 0;
    let mut block_name = String::new();
    let mut style_name = String::new();
    let (mut dx, mut dy, mut dz) = (0.0, 0.0, 0.0);
    let (mut mx, mut my, mut mz) = (0.0, 0.0, 0.0);
    let mut text_override = String::new();
    let mut attachment_point: i16 = 0;
    let mut measurement: f64 = 0.0;
    let mut text_rotation: f64 = 0.0;
    let mut horizontal_direction: f64 = 0.0;
    let mut flip_arrow1 = false;
    let mut flip_arrow2 = false;
    let (mut f1x, mut f1y, mut f1z) = (0.0, 0.0, 0.0);
    let (mut f2x, mut f2y, mut f2z) = (0.0, 0.0, 0.0);
    let (mut ax, mut ay, mut az) = (0.0, 0.0, 0.0);
    let (mut dax, mut day, mut daz) = (0.0, 0.0, 0.0);
    let mut leader_length: f64 = 0.0;
    let mut rotation: f64 = 0.0;
    let mut ext_line_rotation: f64 = 0.0;

    for &(code, ref val) in codes {
        match code {
            70 => dim_type = val.parse().unwrap_or(0),
            2 => block_name = val.clone(),
            3 => style_name = val.clone(),
            1 => text_override = val.clone(),
            10 => dx = val.parse().unwrap_or(0.0),
            20 => dy = val.parse().unwrap_or(0.0),
            30 => dz = val.parse().unwrap_or(0.0),
            11 => mx = val.parse().unwrap_or(0.0),
            21 => my = val.parse().unwrap_or(0.0),
            31 => mz = val.parse().unwrap_or(0.0),
            13 => f1x = val.parse().unwrap_or(0.0),
            23 => f1y = val.parse().unwrap_or(0.0),
            33 => f1z = val.parse().unwrap_or(0.0),
            14 => f2x = val.parse().unwrap_or(0.0),
            24 => f2y = val.parse().unwrap_or(0.0),
            34 => f2z = val.parse().unwrap_or(0.0),
            15 => ax = val.parse().unwrap_or(0.0),
            25 => ay = val.parse().unwrap_or(0.0),
            35 => az = val.parse().unwrap_or(0.0),
            16 => dax = val.parse().unwrap_or(0.0),
            26 => day = val.parse().unwrap_or(0.0),
            36 => daz = val.parse().unwrap_or(0.0),
            40 => leader_length = val.parse().unwrap_or(0.0),
            42 => measurement = val.parse().unwrap_or(0.0),
            50 => rotation = val.parse().unwrap_or(0.0),
            51 => horizontal_direction = val.parse().unwrap_or(0.0),
            52 => ext_line_rotation = val.parse().unwrap_or(0.0),
            53 => text_rotation = val.parse().unwrap_or(0.0),
            71 => attachment_point = val.parse().unwrap_or(0),
            74 => flip_arrow1 = val.parse::<i16>().unwrap_or(0) != 0,
            75 => flip_arrow2 = val.parse::<i16>().unwrap_or(0) != 0,
            _ => {}
        }
    }
    EntityData::Dimension {
        dim_type,
        block_name,
        style_name,
        definition_point: [dx, dy, dz],
        text_midpoint: [mx, my, mz],
        text_override,
        attachment_point,
        measurement,
        text_rotation,
        horizontal_direction,
        flip_arrow1,
        flip_arrow2,
        first_point: [f1x, f1y, f1z],
        second_point: [f2x, f2y, f2z],
        angle_vertex: [ax, ay, az],
        dimension_arc: [dax, day, daz],
        leader_length,
        rotation,
        ext_line_rotation,
    }
}

pub(crate) fn parse_hatch(codes: &[(i16, String)]) -> EntityData {
    use h7cad_native_model::{HatchBoundaryPath, HatchEdge};

    let mut pattern_name = String::new();
    let mut solid_fill = false;
    let mut boundary_paths = Vec::new();

    let mut i = 0;
    while i < codes.len() {
        let (code, ref val) = codes[i];
        match code {
            2 => pattern_name = val.clone(),
            70 => solid_fill = val.parse::<i16>().unwrap_or(0) == 1,
            91 => {
                // Number of boundary paths; actual paths follow via code 92
            }
            92 => {
                let flags: i32 = val.parse().unwrap_or(0);
                let is_polyline = flags & 2 != 0;
                let mut edges = Vec::new();
                i += 1;

                if is_polyline {
                    let mut closed = false;
                    let mut vertices: Vec<[f64; 3]> = Vec::new();
                    let (mut x, mut y) = (0.0, 0.0);
                    let mut bulge = 0.0;
                    let mut has_vertex = false;

                    while i < codes.len() {
                        let (c, ref v) = codes[i];
                        match c {
                            73 => closed = v.parse::<i16>().unwrap_or(0) != 0,
                            10 => {
                                if has_vertex {
                                    vertices.push([x, y, bulge]);
                                    bulge = 0.0;
                                }
                                x = v.parse().unwrap_or(0.0);
                                has_vertex = true;
                            }
                            20 => y = v.parse().unwrap_or(0.0),
                            42 => bulge = v.parse().unwrap_or(0.0),
                            92 | 75 | 76 | 47 | 98 => {
                                if has_vertex {
                                    vertices.push([x, y, bulge]);
                                }
                                i -= 1;
                                break;
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                    if has_vertex && (vertices.is_empty() || *vertices.last().unwrap() != [x, y, bulge]) {
                        vertices.push([x, y, bulge]);
                    }
                    edges.push(HatchEdge::Polyline { closed, vertices });
                } else {
                    let mut num_edges: i32 = 0;
                    // Find code 93 for number of edges
                    while i < codes.len() {
                        let (c, ref v) = codes[i];
                        if c == 93 {
                            num_edges = v.parse().unwrap_or(0);
                            i += 1;
                            break;
                        }
                        i += 1;
                    }

                    for _ in 0..num_edges {
                        if i >= codes.len() {
                            break;
                        }
                        // Code 72 = edge type
                        if codes[i].0 != 72 {
                            break;
                        }
                        let edge_type: i16 = codes[i].1.parse().unwrap_or(0);
                        i += 1;

                        match edge_type {
                            1 => {
                                // Line
                                let (mut sx, mut sy, mut ex, mut ey) = (0.0, 0.0, 0.0, 0.0);
                                while i < codes.len() {
                                    match codes[i].0 {
                                        10 => sx = codes[i].1.parse().unwrap_or(0.0),
                                        20 => sy = codes[i].1.parse().unwrap_or(0.0),
                                        11 => ex = codes[i].1.parse().unwrap_or(0.0),
                                        21 => ey = codes[i].1.parse().unwrap_or(0.0),
                                        72 | 92 | 97 => break,
                                        _ => {}
                                    }
                                    i += 1;
                                }
                                edges.push(HatchEdge::Line {
                                    start: [sx, sy],
                                    end: [ex, ey],
                                });
                            }
                            2 => {
                                // Circular arc
                                let (mut cx, mut cy) = (0.0, 0.0);
                                let mut r = 0.0;
                                let (mut sa, mut ea) = (0.0, 360.0);
                                let mut ccw = true;
                                while i < codes.len() {
                                    match codes[i].0 {
                                        10 => cx = codes[i].1.parse().unwrap_or(0.0),
                                        20 => cy = codes[i].1.parse().unwrap_or(0.0),
                                        40 => r = codes[i].1.parse().unwrap_or(0.0),
                                        50 => sa = codes[i].1.parse().unwrap_or(0.0),
                                        51 => ea = codes[i].1.parse().unwrap_or(0.0),
                                        73 => ccw = codes[i].1.parse::<i16>().unwrap_or(1) != 0,
                                        72 | 92 | 97 => break,
                                        _ => {}
                                    }
                                    i += 1;
                                }
                                edges.push(HatchEdge::CircularArc {
                                    center: [cx, cy],
                                    radius: r,
                                    start_angle: sa,
                                    end_angle: ea,
                                    is_ccw: ccw,
                                });
                            }
                            3 => {
                                // Elliptic arc
                                let (mut cx, mut cy) = (0.0, 0.0);
                                let (mut mx, mut my) = (1.0, 0.0);
                                let mut ratio = 1.0;
                                let (mut sa, mut ea) = (0.0, std::f64::consts::TAU);
                                let mut ccw = true;
                                while i < codes.len() {
                                    match codes[i].0 {
                                        10 => cx = codes[i].1.parse().unwrap_or(0.0),
                                        20 => cy = codes[i].1.parse().unwrap_or(0.0),
                                        11 => mx = codes[i].1.parse().unwrap_or(0.0),
                                        21 => my = codes[i].1.parse().unwrap_or(0.0),
                                        40 => ratio = codes[i].1.parse().unwrap_or(1.0),
                                        50 => sa = codes[i].1.parse().unwrap_or(0.0),
                                        51 => ea = codes[i].1.parse().unwrap_or(0.0),
                                        73 => ccw = codes[i].1.parse::<i16>().unwrap_or(1) != 0,
                                        72 | 92 | 97 => break,
                                        _ => {}
                                    }
                                    i += 1;
                                }
                                edges.push(HatchEdge::EllipticArc {
                                    center: [cx, cy],
                                    major_endpoint: [mx, my],
                                    minor_ratio: ratio,
                                    start_angle: sa,
                                    end_angle: ea,
                                    is_ccw: ccw,
                                });
                            }
                            _ => {
                                // Spline or unknown — skip codes until next edge/boundary
                                while i < codes.len() {
                                    if matches!(codes[i].0, 72 | 92 | 97) {
                                        break;
                                    }
                                    i += 1;
                                }
                            }
                        }
                    }
                }

                boundary_paths.push(HatchBoundaryPath { flags, edges });
                continue;
            }
            _ => {}
        }
        i += 1;
    }

    EntityData::Hatch {
        pattern_name,
        solid_fill,
        boundary_paths,
    }
}

pub(crate) fn parse_viewport(codes: &[(i16, String)]) -> EntityData {
    let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
    let mut w = 0.0;
    let mut h = 0.0;
    for &(code, ref val) in codes {
        let v: f64 = val.parse().unwrap_or(0.0);
        match code {
            10 => cx = v,
            20 => cy = v,
            30 => cz = v,
            40 => w = v,
            41 => h = v,
            _ => {}
        }
    }
    EntityData::Viewport {
        center: [cx, cy, cz],
        width: w,
        height: h,
    }
}

pub(crate) fn parse_text(codes: &[(i16, String)]) -> EntityData {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let mut height = 0.0;
    let mut rotation = 0.0;
    let mut value = String::new();
    for &(code, ref val) in codes {
        match code {
            10 => x = val.parse().unwrap_or(0.0),
            20 => y = val.parse().unwrap_or(0.0),
            30 => z = val.parse().unwrap_or(0.0),
            40 => height = val.parse().unwrap_or(0.0),
            50 => rotation = val.parse().unwrap_or(0.0),
            1 => value = val.clone(),
            _ => {}
        }
    }
    EntityData::Text {
        insertion: [x, y, z],
        height,
        value,
        rotation,
    }
}
