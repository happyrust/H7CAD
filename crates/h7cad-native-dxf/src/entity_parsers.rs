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
    let mut closed = false;
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let mut has_vertex = false;
    for &(code, ref val) in codes {
        match code {
            2 => style_name = val.clone(),
            40 => scale = val.parse().unwrap_or(1.0),
            // Code 71 = MLineFlags bitfield (HAS_VERTICES=1, CLOSED=2,
            // NO_START_CAPS=4, NO_END_CAPS=8); we track closed only.
            71 => closed = (val.parse::<i16>().unwrap_or(0) & 2) != 0,
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
        closed,
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
        name: String::new(),
        rotation: 0.0,
        relative_x_scale: 1.0,
        oblique_angle: 0.0,
        style_name: String::new(),
        normal: [0.0, 0.0, 1.0],
        thickness: 0.0,
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
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut face_indices: Vec<i32> = Vec::new();
    let (mut vx, mut vy) = (0.0_f64, 0.0_f64);

    let mut in_faces = false;

    for &(code, ref val) in codes {
        match code {
            91 => { /* subdivision level */ }
            92 => {
                vertex_count = val.parse().unwrap_or(0);
                in_faces = false;
            }
            10 => vx = val.parse().unwrap_or(0.0),
            20 => vy = val.parse().unwrap_or(0.0),
            30 => {
                let vz: f64 = val.parse().unwrap_or(0.0);
                vertices.push([vx, vy, vz]);
            }
            93 => {
                face_count = val.parse().unwrap_or(0);
                in_faces = true;
            }
            90 => {
                if in_faces {
                    face_indices.push(val.parse().unwrap_or(0));
                }
            }
            _ => {}
        }
    }
    EntityData::Mesh {
        vertex_count,
        face_count,
        vertices,
        face_indices,
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
    let mut weights: Vec<f64> = Vec::new();
    let mut control_points: Vec<[f64; 3]> = Vec::new();
    let mut fit_points: Vec<[f64; 3]> = Vec::new();
    let mut start_tangent = [0.0f64; 3];
    let mut end_tangent = [0.0f64; 3];
    let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
    let (mut fx, mut fy, mut fz) = (0.0, 0.0, 0.0);
    let mut in_control = false;
    let mut in_fit = false;

    for &(code, ref val) in codes {
        match code {
            71 => degree = val.parse().unwrap_or(3),
            70 => closed = val.parse::<i16>().unwrap_or(0) & 1 != 0,
            40 => knots.push(val.parse().unwrap_or(0.0)),
            41 => weights.push(val.parse().unwrap_or(1.0)),
            12 => start_tangent[0] = val.parse().unwrap_or(0.0),
            22 => start_tangent[1] = val.parse().unwrap_or(0.0),
            32 => start_tangent[2] = val.parse().unwrap_or(0.0),
            13 => end_tangent[0] = val.parse().unwrap_or(0.0),
            23 => end_tangent[1] = val.parse().unwrap_or(0.0),
            33 => end_tangent[2] = val.parse().unwrap_or(0.0),
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
        weights,
        fit_points,
        start_tangent,
        end_tangent,
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
    EntityData::Face3D {
        corners,
        invisible_edges: 0,
    }
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
    EntityData::Solid {
        corners,
        normal: [0.0, 0.0, 1.0],
        thickness: 0.0,
    }
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
    let mut rectangle_height = None;
    let mut rotation = 0.0;
    let mut value = String::new();
    let mut style_name = String::new();
    let mut attachment_point: i16 = 0;
    let mut line_spacing_factor = 1.0;
    let mut drawing_direction: i16 = 5;
    for &(code, ref val) in codes {
        match code {
            10 => x = val.parse().unwrap_or(0.0),
            20 => y = val.parse().unwrap_or(0.0),
            30 => z = val.parse().unwrap_or(0.0),
            40 => height = val.parse().unwrap_or(0.0),
            41 => width = val.parse().unwrap_or(0.0),
            43 => rectangle_height = Some(val.parse().unwrap_or(0.0)),
            44 => line_spacing_factor = val.parse().unwrap_or(1.0),
            50 => rotation = val.parse().unwrap_or(0.0),
            71 => attachment_point = val.parse().unwrap_or(0),
            72 => drawing_direction = val.parse().unwrap_or(5),
            7 => style_name = val.clone(),
            1 | 3 => value.push_str(val),
            _ => {}
        }
    }
    EntityData::MText {
        insertion: [x, y, z],
        height,
        width,
        rectangle_height,
        value,
        rotation,
        style_name,
        attachment_point,
        line_spacing_factor,
        drawing_direction,
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
    let (mut ax, mut ay, mut az) = (0.0, 0.0, 0.0);
    let mut height = 0.0;
    let mut rotation = 0.0;
    let mut value = String::new();
    let mut style_name = String::new();
    let mut width_factor = 1.0;
    let mut oblique_angle = 0.0;
    let mut horizontal_alignment: i16 = 0;
    let mut vertical_alignment: i16 = 0;
    let mut has_alignment_point = false;
    for &(code, ref val) in codes {
        match code {
            10 => x = val.parse().unwrap_or(0.0),
            20 => y = val.parse().unwrap_or(0.0),
            30 => z = val.parse().unwrap_or(0.0),
            11 => {
                ax = val.parse().unwrap_or(0.0);
                has_alignment_point = true;
            }
            21 => ay = val.parse().unwrap_or(0.0),
            31 => az = val.parse().unwrap_or(0.0),
            40 => height = val.parse().unwrap_or(0.0),
            41 => width_factor = val.parse().unwrap_or(1.0),
            50 => rotation = val.parse().unwrap_or(0.0),
            51 => oblique_angle = val.parse().unwrap_or(0.0),
            72 => horizontal_alignment = val.parse().unwrap_or(0),
            73 => vertical_alignment = val.parse().unwrap_or(0),
            1 => value = val.clone(),
            7 => style_name = val.clone(),
            _ => {}
        }
    }
    EntityData::Text {
        insertion: [x, y, z],
        height,
        value,
        rotation,
        style_name,
        width_factor,
        oblique_angle,
        horizontal_alignment,
        vertical_alignment,
        alignment_point: has_alignment_point.then_some([ax, ay, az]),
    }
}

pub(crate) fn parse_acad_table(codes: &[(i16, String)]) -> EntityData {
    let mut num_rows: i32 = 0;
    let mut num_cols: i32 = 0;
    let mut insertion = [0.0f64; 3];
    let mut horizontal_direction = [1.0f64, 0.0, 0.0];
    let mut version: i16 = 0;
    let mut value_flag: i32 = 0;
    for &(code, ref val) in codes {
        match code {
            91 => num_rows = val.parse().unwrap_or(0),
            92 => num_cols = val.parse().unwrap_or(0),
            10 => insertion[0] = val.parse().unwrap_or(0.0),
            20 => insertion[1] = val.parse().unwrap_or(0.0),
            30 => insertion[2] = val.parse().unwrap_or(0.0),
            11 => horizontal_direction[0] = val.parse().unwrap_or(1.0),
            21 => horizontal_direction[1] = val.parse().unwrap_or(0.0),
            31 => horizontal_direction[2] = val.parse().unwrap_or(0.0),
            280 => version = val.parse().unwrap_or(0),
            90 => value_flag = val.parse().unwrap_or(0),
            _ => {}
        }
    }
    EntityData::Table {
        num_rows,
        num_cols,
        insertion,
        horizontal_direction,
        version,
        value_flag,
    }
}

pub(crate) fn parse_multileader(codes: &[(i16, String)]) -> EntityData {
    let mut content_type: i16 = 0;
    let mut text_label = String::new();
    let mut style_name = String::new();
    let mut arrowhead_size = 0.0;
    let mut landing_gap = 0.0;
    let mut dogleg_length = 0.0;
    let mut property_override_flags: u32 = 0;
    let mut path_type: i16 = 1;
    let mut line_color: i32 = 0;
    let mut leader_line_weight: i16 = -1;
    let mut enable_landing = true;
    let mut enable_dogleg = true;
    let mut enable_annotation_scale = false;
    let mut scale_factor = 1.0;
    let mut text_attachment_direction: i16 = 0;
    let mut text_bottom_attachment_type: i16 = 9;
    let mut text_top_attachment_type: i16 = 9;
    let mut text_location: Option<[f64; 3]> = None;
    let mut leader_vertices: Vec<[f64; 3]> = Vec::new();
    let mut leader_root_lengths: Vec<usize> = Vec::new();
    let mut in_context_data = false;
    let mut in_leader_line = false;
    let mut ctx_x = 0.0f64;
    let mut ctx_y = 0.0f64;
    let mut lv_x = 0.0f64;
    let mut lv_y = 0.0f64;
    let mut current_leader_len = 0usize;
    for &(code, ref val) in codes {
        if code == 300 && val.trim() == "CONTEXT_DATA{" {
            in_context_data = true;
            continue;
        }
        if code == 302 && val.trim() == "LEADER_LINE{" {
            if in_leader_line && current_leader_len > 0 {
                leader_root_lengths.push(current_leader_len);
                current_leader_len = 0;
            }
            in_leader_line = true;
            continue;
        }
        if code == 303 || code == 304 && val.trim().ends_with('}') {
            if in_leader_line {
                if current_leader_len > 0 {
                    leader_root_lengths.push(current_leader_len);
                    current_leader_len = 0;
                }
                in_leader_line = false;
            } else {
                in_context_data = false;
            }
        }
        if in_leader_line {
            match code {
                10 => lv_x = val.parse().unwrap_or(0.0),
                20 => lv_y = val.parse().unwrap_or(0.0),
                30 => {
                    let z: f64 = val.parse().unwrap_or(0.0);
                    leader_vertices.push([lv_x, lv_y, z]);
                    current_leader_len += 1;
                }
                _ => {}
            }
            continue;
        }
        if in_context_data {
            match code {
                12 => ctx_x = val.parse().unwrap_or(0.0),
                22 => ctx_y = val.parse().unwrap_or(0.0),
                32 => {
                    let z: f64 = val.parse().unwrap_or(0.0);
                    text_location = Some([ctx_x, ctx_y, z]);
                }
                _ => {}
            }
            continue;
        }
        match code {
            172 => content_type = val.parse().unwrap_or(0),
            304 => text_label = val.clone(),
            3 => style_name = val.clone(),
            42 => arrowhead_size = val.parse().unwrap_or(0.0),
            41 => landing_gap = val.parse().unwrap_or(0.0),
            43 => dogleg_length = val.parse().unwrap_or(0.0),
            90 => property_override_flags = val.parse().unwrap_or(0),
            170 => path_type = val.parse().unwrap_or(1),
            91 => line_color = val.parse().unwrap_or(0),
            171 => leader_line_weight = val.parse().unwrap_or(-1),
            290 => enable_landing = val.trim() == "1",
            291 => enable_dogleg = val.trim() == "1",
            293 => enable_annotation_scale = val.trim() == "1",
            45 => scale_factor = val.parse().unwrap_or(1.0),
            271 => text_attachment_direction = val.parse().unwrap_or(0),
            272 => text_bottom_attachment_type = val.parse().unwrap_or(9),
            273 => text_top_attachment_type = val.parse().unwrap_or(9),
            _ => {}
        }
    }
    EntityData::MultiLeader {
        content_type,
        text_label,
        style_name,
        arrowhead_size,
        landing_gap,
        dogleg_length,
        property_override_flags,
        path_type,
        line_color,
        leader_line_weight,
        enable_landing,
        enable_dogleg,
        enable_annotation_scale,
        scale_factor,
        text_attachment_direction,
        text_bottom_attachment_type,
        text_top_attachment_type,
        text_location,
        leader_vertices,
        leader_root_lengths,
    }
}

pub(crate) fn parse_helix(codes: &[(i16, String)]) -> EntityData {
    let (mut ax, mut ay, mut az) = (0.0, 0.0, 0.0);
    let (mut sx, mut sy, mut sz) = (0.0, 0.0, 0.0);
    let (mut vx, mut vy, mut vz) = (0.0, 0.0, 1.0);
    let mut radius = 0.0;
    let mut turns = 0.0;
    let mut turn_height = 0.0;
    let mut handedness: i16 = 0;
    let mut is_ccw = true;
    for &(code, ref val) in codes {
        match code {
            10 => ax = val.parse().unwrap_or(0.0),
            20 => ay = val.parse().unwrap_or(0.0),
            30 => az = val.parse().unwrap_or(0.0),
            11 => sx = val.parse().unwrap_or(0.0),
            21 => sy = val.parse().unwrap_or(0.0),
            31 => sz = val.parse().unwrap_or(0.0),
            12 => vx = val.parse().unwrap_or(0.0),
            22 => vy = val.parse().unwrap_or(0.0),
            32 => vz = val.parse().unwrap_or(1.0),
            40 => radius = val.parse().unwrap_or(0.0),
            41 => turns = val.parse().unwrap_or(0.0),
            42 => turn_height = val.parse().unwrap_or(0.0),
            280 => handedness = val.parse().unwrap_or(0),
            290 => is_ccw = val.trim() == "1",
            _ => {}
        }
    }
    EntityData::Helix {
        axis_base_point: [ax, ay, az],
        start_point: [sx, sy, sz],
        axis_vector: [vx, vy, vz],
        radius,
        turns,
        turn_height,
        handedness,
        is_ccw,
    }
}

pub(crate) fn parse_arc_dimension(codes: &[(i16, String)]) -> EntityData {
    let mut block_name = String::new();
    let mut style_name = String::new();
    let mut def = [0.0; 3];
    let mut mid = [0.0; 3];
    let mut text_override = String::new();
    let mut first = [0.0; 3];
    let mut second = [0.0; 3];
    let mut center = [0.0; 3];
    let mut leader_length = 0.0;
    let mut measurement = 0.0;
    for &(code, ref val) in codes {
        match code {
            2 => block_name = val.clone(),
            3 => style_name = val.clone(),
            1 => text_override = val.clone(),
            10 => def[0] = val.parse().unwrap_or(0.0),
            20 => def[1] = val.parse().unwrap_or(0.0),
            30 => def[2] = val.parse().unwrap_or(0.0),
            11 => mid[0] = val.parse().unwrap_or(0.0),
            21 => mid[1] = val.parse().unwrap_or(0.0),
            31 => mid[2] = val.parse().unwrap_or(0.0),
            13 => first[0] = val.parse().unwrap_or(0.0),
            23 => first[1] = val.parse().unwrap_or(0.0),
            33 => first[2] = val.parse().unwrap_or(0.0),
            14 => second[0] = val.parse().unwrap_or(0.0),
            24 => second[1] = val.parse().unwrap_or(0.0),
            34 => second[2] = val.parse().unwrap_or(0.0),
            15 => center[0] = val.parse().unwrap_or(0.0),
            25 => center[1] = val.parse().unwrap_or(0.0),
            35 => center[2] = val.parse().unwrap_or(0.0),
            40 => leader_length = val.parse().unwrap_or(0.0),
            42 => measurement = val.parse().unwrap_or(0.0),
            _ => {}
        }
    }
    EntityData::ArcDimension {
        block_name,
        style_name,
        definition_point: def,
        text_midpoint: mid,
        text_override,
        first_point: first,
        second_point: second,
        arc_center: center,
        leader_length,
        measurement,
    }
}

pub(crate) fn parse_large_radial_dimension(codes: &[(i16, String)]) -> EntityData {
    let mut block_name = String::new();
    let mut style_name = String::new();
    let mut def = [0.0; 3];
    let mut mid = [0.0; 3];
    let mut text_override = String::new();
    let mut chord = [0.0; 3];
    let mut leader_length = 0.0;
    let mut jog_angle = 0.0;
    let mut measurement = 0.0;
    for &(code, ref val) in codes {
        match code {
            2 => block_name = val.clone(),
            3 => style_name = val.clone(),
            1 => text_override = val.clone(),
            10 => def[0] = val.parse().unwrap_or(0.0),
            20 => def[1] = val.parse().unwrap_or(0.0),
            30 => def[2] = val.parse().unwrap_or(0.0),
            11 => mid[0] = val.parse().unwrap_or(0.0),
            21 => mid[1] = val.parse().unwrap_or(0.0),
            31 => mid[2] = val.parse().unwrap_or(0.0),
            15 => chord[0] = val.parse().unwrap_or(0.0),
            25 => chord[1] = val.parse().unwrap_or(0.0),
            35 => chord[2] = val.parse().unwrap_or(0.0),
            40 => leader_length = val.parse().unwrap_or(0.0),
            50 => jog_angle = val.parse().unwrap_or(0.0),
            42 => measurement = val.parse().unwrap_or(0.0),
            _ => {}
        }
    }
    EntityData::LargeRadialDimension {
        block_name,
        style_name,
        definition_point: def,
        text_midpoint: mid,
        text_override,
        chord_point: chord,
        leader_length,
        jog_angle,
        measurement,
    }
}

pub(crate) fn parse_surface(codes: &[(i16, String)], surface_kind: &str) -> EntityData {
    let mut u_isolines = 0;
    let mut v_isolines = 0;
    let mut acis_lines: Vec<String> = Vec::new();
    for &(code, ref val) in codes {
        match code {
            70 => u_isolines = val.parse().unwrap_or(0),
            71 => v_isolines = val.parse().unwrap_or(0),
            1 | 3 => acis_lines.push(val.clone()),
            _ => {}
        }
    }
    EntityData::Surface {
        surface_kind: surface_kind.to_string(),
        u_isolines,
        v_isolines,
        acis_data: acis_lines.join("\n"),
    }
}

pub(crate) fn parse_light(codes: &[(i16, String)]) -> EntityData {
    let mut name = String::new();
    let mut light_type: i16 = 2;
    let mut position = [0.0; 3];
    let mut target = [0.0; 3];
    let mut intensity = 1.0;
    let mut is_on = true;
    let mut color: i16 = 7;
    let mut hotspot_angle = 0.0;
    let mut falloff_angle = 0.0;
    for &(code, ref val) in codes {
        match code {
            1 => name = val.clone(),
            70 => light_type = val.parse().unwrap_or(2),
            10 => position[0] = val.parse().unwrap_or(0.0),
            20 => position[1] = val.parse().unwrap_or(0.0),
            30 => position[2] = val.parse().unwrap_or(0.0),
            11 => target[0] = val.parse().unwrap_or(0.0),
            21 => target[1] = val.parse().unwrap_or(0.0),
            31 => target[2] = val.parse().unwrap_or(0.0),
            40 => intensity = val.parse().unwrap_or(1.0),
            290 => is_on = val.trim() == "1",
            63 => color = val.parse().unwrap_or(7),
            50 => hotspot_angle = val.parse().unwrap_or(0.0),
            51 => falloff_angle = val.parse().unwrap_or(0.0),
            _ => {}
        }
    }
    EntityData::Light {
        name,
        light_type,
        position,
        target,
        intensity,
        is_on,
        color,
        hotspot_angle,
        falloff_angle,
    }
}

pub(crate) fn parse_camera(codes: &[(i16, String)]) -> EntityData {
    let mut position = [0.0; 3];
    let mut target = [0.0; 3];
    let mut lens_length = 50.0;
    for &(code, ref val) in codes {
        match code {
            10 => position[0] = val.parse().unwrap_or(0.0),
            20 => position[1] = val.parse().unwrap_or(0.0),
            30 => position[2] = val.parse().unwrap_or(0.0),
            11 => target[0] = val.parse().unwrap_or(0.0),
            21 => target[1] = val.parse().unwrap_or(0.0),
            31 => target[2] = val.parse().unwrap_or(0.0),
            40 => lens_length = val.parse().unwrap_or(50.0),
            _ => {}
        }
    }
    EntityData::Camera {
        position,
        target,
        lens_length,
    }
}

pub(crate) fn parse_section(codes: &[(i16, String)]) -> EntityData {
    let mut name = String::new();
    let mut state = 0;
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut cur = [0.0; 3];
    let mut have_x = false;
    let mut have_y = false;
    let mut vertical = [0.0, 0.0, 1.0];
    for &(code, ref val) in codes {
        match code {
            1 => name = val.clone(),
            70 => state = val.parse().unwrap_or(0),
            11 => {
                cur[0] = val.parse().unwrap_or(0.0);
                have_x = true;
                have_y = false;
            }
            21 => {
                cur[1] = val.parse().unwrap_or(0.0);
                have_y = have_x;
            }
            31 => {
                cur[2] = val.parse().unwrap_or(0.0);
                if have_y {
                    vertices.push(cur);
                }
                have_x = false;
                have_y = false;
            }
            40 => vertical[0] = val.parse().unwrap_or(0.0),
            41 => vertical[1] = val.parse().unwrap_or(0.0),
            42 => vertical[2] = val.parse().unwrap_or(1.0),
            _ => {}
        }
    }
    EntityData::Section {
        name,
        state,
        vertices,
        vertical_direction: vertical,
    }
}

pub(crate) fn parse_proxy_entity(codes: &[(i16, String)]) -> EntityData {
    let mut class_id: i32 = 0;
    let mut application_class_id: i32 = 0;
    let mut raw_codes: Vec<(i16, String)> = Vec::new();
    for &(code, ref val) in codes {
        match code {
            90 if class_id == 0 => class_id = val.parse().unwrap_or(0),
            91 if application_class_id == 0 => application_class_id = val.parse().unwrap_or(0),
            // Skip fields already handled by the common entity header.
            5 | 100 | 330 | 8 | 6 | 48 | 62 | 420 | 370 | 60 | 440 | 39 | 210 | 220 | 230 => {}
            _ => raw_codes.push((code, val.clone())),
        }
    }
    EntityData::ProxyEntity {
        class_id,
        application_class_id,
        raw_codes,
    }
}
