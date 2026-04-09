use h7cad_native_model::*;
use std::fmt::Write;

pub struct DxfWriter {
    buf: String,
}

impl DxfWriter {
    pub fn new() -> Self {
        Self {
            buf: String::with_capacity(64 * 1024),
        }
    }

    pub fn finish(self) -> String {
        self.buf
    }

    fn pair(&mut self, code: i16, value: &str) {
        writeln!(self.buf, "{:>3}", code).unwrap();
        writeln!(self.buf, "{}", value).unwrap();
    }

    fn pair_str(&mut self, code: i16, value: &str) {
        self.pair(code, value);
    }

    fn pair_i16(&mut self, code: i16, value: i16) {
        self.pair(code, &format!("{:>6}", value));
    }

    fn pair_i32(&mut self, code: i16, value: i32) {
        self.pair(code, &value.to_string());
    }

    fn pair_f64(&mut self, code: i16, value: f64) {
        self.pair(code, &format_f64(value));
    }

    fn pair_handle(&mut self, code: i16, handle: Handle) {
        self.pair(code, &format!("{:X}", handle.value()));
    }

    fn point3d(&mut self, base_code: i16, p: [f64; 3]) {
        self.pair_f64(base_code, p[0]);
        self.pair_f64(base_code + 10, p[1]);
        self.pair_f64(base_code + 20, p[2]);
    }

    fn point2d(&mut self, base_code: i16, p: [f64; 2]) {
        self.pair_f64(base_code, p[0]);
        self.pair_f64(base_code + 10, p[1]);
    }
}

fn format_f64(v: f64) -> String {
    if v == 0.0 {
        "0.0".into()
    } else {
        let s = format!("{:.10}", v);
        let s = s.trim_end_matches('0');
        if s.ends_with('.') {
            format!("{s}0")
        } else {
            s.to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn write_dxf_string(doc: &CadDocument) -> Result<String, String> {
    let mut w = DxfWriter::new();

    write_header(&mut w, doc);
    write_classes(&mut w, doc);
    write_tables(&mut w, doc);
    write_blocks(&mut w, doc);
    write_entities(&mut w, doc);
    write_objects(&mut w, doc);

    w.pair_str(0, "EOF");
    Ok(w.finish())
}

// ---------------------------------------------------------------------------
// HEADER
// ---------------------------------------------------------------------------

fn write_header(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "HEADER");

    w.pair_str(9, "$ACADVER");
    w.pair_str(1, doc.header.version.to_dxf());

    w.pair_str(9, "$INSBASE");
    w.point3d(10, doc.header.insbase);

    w.pair_str(9, "$EXTMIN");
    w.point3d(10, doc.header.extmin);

    w.pair_str(9, "$EXTMAX");
    w.point3d(10, doc.header.extmax);

    w.pair_str(9, "$LIMMIN");
    w.point2d(10, doc.header.limmin);

    w.pair_str(9, "$LIMMAX");
    w.point2d(10, doc.header.limmax);

    w.pair_str(9, "$LTSCALE");
    w.pair_f64(40, doc.header.ltscale);

    w.pair_str(9, "$TEXTSIZE");
    w.pair_f64(40, doc.header.textsize);

    w.pair_str(9, "$DIMSCALE");
    w.pair_f64(40, doc.header.dimscale);

    w.pair_str(9, "$PDMODE");
    w.pair_i32(70, doc.header.pdmode);

    w.pair_str(9, "$PDSIZE");
    w.pair_f64(40, doc.header.pdsize);

    w.pair_str(9, "$LUNITS");
    w.pair_i16(70, doc.header.lunits);

    w.pair_str(9, "$LUPREC");
    w.pair_i16(70, doc.header.luprec);

    w.pair_str(9, "$AUNITS");
    w.pair_i16(70, doc.header.aunits);

    w.pair_str(9, "$AUPREC");
    w.pair_i16(70, doc.header.auprec);

    w.pair_str(9, "$HANDSEED");
    w.pair_str(5, &format!("{:X}", doc.next_handle()));

    w.pair_str(0, "ENDSEC");
}

// ---------------------------------------------------------------------------
// CLASSES
// ---------------------------------------------------------------------------

fn write_classes(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "CLASSES");

    for cls in &doc.classes {
        w.pair_str(0, "CLASS");
        w.pair_str(1, &cls.dxf_name);
        w.pair_str(2, &cls.cpp_class_name);
        w.pair_str(3, &cls.application_name);
        w.pair_i32(90, cls.proxy_flags);
        w.pair_i32(91, cls.instance_count);
        w.pair_i16(
            280,
            if cls.was_a_proxy { 1 } else { 0 },
        );
        w.pair_i16(
            281,
            if cls.is_an_entity { 1 } else { 0 },
        );
    }

    w.pair_str(0, "ENDSEC");
}

// ---------------------------------------------------------------------------
// TABLES
// ---------------------------------------------------------------------------

fn write_tables(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "TABLES");

    write_vport_table(w);
    write_ltype_table(w, doc);
    write_layer_table(w, doc);
    write_style_table(w, doc);
    write_dimstyle_table(w, doc);
    write_block_record_table(w, doc);

    w.pair_str(0, "ENDSEC");
}

fn write_vport_table(w: &mut DxfWriter) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "VPORT");
    w.pair_i16(70, 0);
    w.pair_str(0, "ENDTAB");
}

fn write_ltype_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "LTYPE");
    w.pair_i16(70, doc.linetypes.len() as i16);

    for lt in doc.linetypes.values() {
        w.pair_str(0, "LTYPE");
        if lt.handle != Handle::NULL {
            w.pair_handle(5, lt.handle);
        }
        w.pair_str(2, &lt.name);
        w.pair_i16(70, 0);
        w.pair_str(3, &lt.description);
        w.pair_i16(72, 65);
        w.pair_i16(73, lt.segments.len() as i16);
        w.pair_f64(40, lt.pattern_length);
        for seg in &lt.segments {
            w.pair_f64(49, seg.length);
        }
    }

    w.pair_str(0, "ENDTAB");
}

fn write_layer_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "LAYER");
    w.pair_i16(70, doc.layers.len() as i16);

    for layer in doc.layers.values() {
        w.pair_str(0, "LAYER");
        if layer.handle != Handle::NULL {
            w.pair_handle(5, layer.handle);
        }
        w.pair_str(2, &layer.name);
        let mut flags: i16 = 0;
        if layer.is_frozen {
            flags |= 1;
        }
        if layer.is_locked {
            flags |= 4;
        }
        w.pair_i16(70, flags);
        w.pair_i16(62, layer.color);
        w.pair_str(6, &layer.linetype_name);
        if layer.lineweight != -1 {
            w.pair_i16(370, layer.lineweight);
        }
        if !layer.plot {
            w.pair_i16(290, 0);
        }
        if layer.true_color != 0 {
            w.pair_i32(420, layer.true_color);
        }
    }

    w.pair_str(0, "ENDTAB");
}

fn write_style_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "STYLE");
    w.pair_i16(70, doc.text_styles.len() as i16);

    for ts in doc.text_styles.values() {
        w.pair_str(0, "STYLE");
        if ts.handle != Handle::NULL {
            w.pair_handle(5, ts.handle);
        }
        w.pair_str(2, &ts.name);
        w.pair_i16(70, ts.flags);
        w.pair_f64(40, ts.height);
        w.pair_f64(41, ts.width_factor);
        w.pair_f64(50, ts.oblique_angle);
        w.pair_str(3, &ts.font_name);
        if !ts.bigfont_name.is_empty() {
            w.pair_str(4, &ts.bigfont_name);
        }
    }

    w.pair_str(0, "ENDTAB");
}

fn write_dimstyle_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "DIMSTYLE");
    w.pair_i16(70, doc.dim_styles.len() as i16);

    for ds in doc.dim_styles.values() {
        w.pair_str(0, "DIMSTYLE");
        if ds.handle != Handle::NULL {
            w.pair_handle(105, ds.handle);
        }
        w.pair_str(2, &ds.name);
        w.pair_i16(70, 0);
        w.pair_f64(40, ds.dimscale);
        w.pair_f64(41, ds.dimasz);
        w.pair_f64(42, ds.dimexo);
        w.pair_f64(44, ds.dimgap);
        w.pair_f64(140, ds.dimtxt);
        w.pair_i16(271, ds.dimdec);
        w.pair_i16(277, ds.dimlunit);
        w.pair_i16(275, ds.dimaunit);
        if !ds.dimtxsty_name.is_empty() {
            w.pair_str(340, &ds.dimtxsty_name);
        }
    }

    w.pair_str(0, "ENDTAB");
}

fn write_block_record_table(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "TABLE");
    w.pair_str(2, "BLOCK_RECORD");
    w.pair_i16(70, doc.block_records.len() as i16);

    for br in doc.block_records.values() {
        w.pair_str(0, "BLOCK_RECORD");
        w.pair_handle(5, br.handle);
        w.pair_str(2, &br.name);
    }

    w.pair_str(0, "ENDTAB");
}

// ---------------------------------------------------------------------------
// BLOCKS
// ---------------------------------------------------------------------------

fn write_blocks(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "BLOCKS");

    for br in doc.block_records.values() {
        w.pair_str(0, "BLOCK");
        if br.block_entity_handle != Handle::NULL {
            w.pair_handle(5, br.block_entity_handle);
        }
        w.pair_handle(330, br.handle);
        w.pair_str(2, &br.name);
        w.pair_i16(70, 0);
        w.point3d(10, [0.0, 0.0, 0.0]);

        for entity in &br.entities {
            write_entity(w, entity);
        }

        w.pair_str(0, "ENDBLK");
        if br.block_entity_handle != Handle::NULL {
            w.pair_handle(5, br.block_entity_handle);
        }
        w.pair_handle(330, br.handle);
    }

    w.pair_str(0, "ENDSEC");
}

// ---------------------------------------------------------------------------
// ENTITIES
// ---------------------------------------------------------------------------

fn write_entities(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "ENTITIES");

    for entity in &doc.entities {
        write_entity(w, entity);
    }

    w.pair_str(0, "ENDSEC");
}

fn write_entity(w: &mut DxfWriter, entity: &Entity) {
    let type_name = entity.data.type_name();
    w.pair_str(0, &type_name);

    if entity.handle != Handle::NULL {
        w.pair_handle(5, entity.handle);
    }
    if entity.owner_handle != Handle::NULL {
        w.pair_handle(330, entity.owner_handle);
    }

    w.pair_str(8, &entity.layer_name);
    if entity.color_index != 256 {
        w.pair_i16(62, entity.color_index);
    }
    if !entity.linetype_name.is_empty() {
        w.pair_str(6, &entity.linetype_name);
    }
    if entity.lineweight != -1 {
        w.pair_i16(370, entity.lineweight);
    }
    if entity.true_color != 0 {
        w.pair_i32(420, entity.true_color);
    }
    if entity.invisible {
        w.pair_i16(60, 1);
    }
    if entity.transparency != 0 {
        w.pair_i32(440, entity.transparency);
    }

    write_entity_data(w, entity);
}

fn write_entity_data(w: &mut DxfWriter, entity: &Entity) {
    match &entity.data {
        EntityData::Line { start, end } => {
            w.point3d(10, *start);
            w.point3d(11, *end);
        }
        EntityData::Circle { center, radius } => {
            w.point3d(10, *center);
            w.pair_f64(40, *radius);
        }
        EntityData::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            w.point3d(10, *center);
            w.pair_f64(40, *radius);
            w.pair_f64(50, *start_angle);
            w.pair_f64(51, *end_angle);
        }
        EntityData::Point { position } => {
            w.point3d(10, *position);
        }
        EntityData::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => {
            w.point3d(10, *center);
            w.point3d(11, *major_axis);
            w.pair_f64(40, *ratio);
            w.pair_f64(41, *start_param);
            w.pair_f64(42, *end_param);
        }
        EntityData::LwPolyline { vertices, closed } => {
            w.pair_i32(90, vertices.len() as i32);
            w.pair_i16(70, if *closed { 1 } else { 0 });
            for v in vertices {
                w.pair_f64(10, v.x);
                w.pair_f64(20, v.y);
                if v.bulge != 0.0 {
                    w.pair_f64(42, v.bulge);
                }
            }
        }
        EntityData::Text {
            insertion,
            height,
            value,
            rotation,
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(40, *height);
            w.pair_str(1, value);
            if *rotation != 0.0 {
                w.pair_f64(50, *rotation);
            }
        }
        EntityData::MText {
            insertion,
            height,
            width,
            value,
            rotation,
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(40, *height);
            w.pair_f64(41, *width);
            w.pair_str(1, value);
            if *rotation != 0.0 {
                w.pair_f64(50, *rotation);
            }
        }
        EntityData::Insert {
            block_name,
            insertion,
            scale,
            rotation,
            has_attribs,
            attribs,
        } => {
            w.pair_str(2, block_name);
            w.point3d(10, *insertion);
            if scale[0] != 1.0 {
                w.pair_f64(41, scale[0]);
            }
            if scale[1] != 1.0 {
                w.pair_f64(42, scale[1]);
            }
            if scale[2] != 1.0 {
                w.pair_f64(43, scale[2]);
            }
            if *rotation != 0.0 {
                w.pair_f64(50, *rotation);
            }
            if *has_attribs {
                w.pair_i16(66, 1);
            }
            for attrib in attribs {
                write_entity(w, attrib);
            }
            if *has_attribs {
                w.pair_str(0, "SEQEND");
                if entity.handle != Handle::NULL {
                    w.pair_handle(5, entity.handle);
                }
            }
        }
        EntityData::Hatch {
            pattern_name,
            solid_fill,
            boundary_paths,
        } => {
            w.pair_str(2, pattern_name);
            w.pair_i16(70, if *solid_fill { 1 } else { 0 });
            w.pair_i32(91, boundary_paths.len() as i32);
            for path in boundary_paths {
                w.pair_i32(92, path.flags);
                if path.flags & 2 != 0 {
                    w.pair_i32(93, path.edges.len() as i32);
                    for edge in &path.edges {
                        write_hatch_edge(w, edge);
                    }
                }
            }
        }
        EntityData::Dimension {
            dim_type,
            block_name,
            style_name,
            definition_point,
            text_midpoint,
            text_override,
            attachment_point,
            measurement,
            text_rotation,
            horizontal_direction,
            flip_arrow1,
            flip_arrow2,
            first_point,
            second_point,
            angle_vertex,
            dimension_arc,
            leader_length,
            rotation,
            ext_line_rotation,
        } => {
            w.pair_str(2, block_name);
            w.pair_str(3, style_name);
            w.point3d(10, *definition_point);
            w.point3d(11, *text_midpoint);
            w.pair_i16(70, *dim_type);
            if !text_override.is_empty() {
                w.pair_str(1, text_override);
            }
            if *attachment_point != 0 {
                w.pair_i16(71, *attachment_point);
            }
            if *measurement != 0.0 {
                w.pair_f64(42, *measurement);
            }
            if *text_rotation != 0.0 {
                w.pair_f64(53, *text_rotation);
            }
            if *horizontal_direction != 0.0 {
                w.pair_f64(51, *horizontal_direction);
            }
            if *flip_arrow1 {
                w.pair_i16(74, 1);
            }
            if *flip_arrow2 {
                w.pair_i16(75, 1);
            }
            w.point3d(13, *first_point);
            w.point3d(14, *second_point);
            if *angle_vertex != [0.0, 0.0, 0.0] {
                w.point3d(15, *angle_vertex);
            }
            if *dimension_arc != [0.0, 0.0, 0.0] {
                w.point3d(16, *dimension_arc);
            }
            if *leader_length != 0.0 {
                w.pair_f64(40, *leader_length);
            }
            if *rotation != 0.0 {
                w.pair_f64(50, *rotation);
            }
            if *ext_line_rotation != 0.0 {
                w.pair_f64(52, *ext_line_rotation);
            }
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
        } => {
            let mut flags: i32 = 0;
            if *closed {
                flags |= 1;
            }
            w.pair_i32(70, flags);
            w.pair_i32(71, *degree);
            w.pair_i32(72, knots.len() as i32);
            w.pair_i32(73, control_points.len() as i32);
            w.pair_i32(74, fit_points.len() as i32);
            if *start_tangent != [0.0, 0.0, 0.0] {
                w.point3d(12, *start_tangent);
            }
            if *end_tangent != [0.0, 0.0, 0.0] {
                w.point3d(13, *end_tangent);
            }
            for k in knots {
                w.pair_f64(40, *k);
            }
            if !weights.is_empty() {
                for wt in weights {
                    w.pair_f64(41, *wt);
                }
            }
            for cp in control_points {
                w.point3d(10, *cp);
            }
            for fp in fit_points {
                w.point3d(11, *fp);
            }
        }
        EntityData::Face3D { corners } => {
            w.point3d(10, corners[0]);
            w.point3d(11, corners[1]);
            w.point3d(12, corners[2]);
            w.point3d(13, corners[3]);
        }
        EntityData::Solid { corners } => {
            w.point3d(10, corners[0]);
            w.point3d(11, corners[1]);
            w.point3d(12, corners[2]);
            w.point3d(13, corners[3]);
        }
        EntityData::Ray { origin, direction } | EntityData::XLine { origin, direction } => {
            w.point3d(10, *origin);
            w.point3d(11, *direction);
        }
        EntityData::Viewport {
            center,
            width,
            height,
        } => {
            w.point3d(10, *center);
            w.pair_f64(40, *width);
            w.pair_f64(41, *height);
        }
        EntityData::Polyline {
            polyline_type,
            vertices,
            closed,
        } => {
            let mut flags: i16 = 0;
            if *closed {
                flags |= 1;
            }
            match polyline_type {
                PolylineType::Polyline3D => flags |= 8,
                PolylineType::PolygonMesh => flags |= 16,
                PolylineType::PolyfaceMesh => flags |= 64,
                _ => {}
            }
            w.pair_i16(70, flags);
            for v in vertices {
                w.pair_str(0, "VERTEX");
                w.pair_str(8, &entity.layer_name);
                w.point3d(10, v.position);
                if v.bulge != 0.0 {
                    w.pair_f64(42, v.bulge);
                }
            }
            w.pair_str(0, "SEQEND");
            w.pair_str(8, &entity.layer_name);
        }
        EntityData::Attrib {
            tag,
            value,
            insertion,
            height,
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(40, *height);
            w.pair_str(1, value);
            w.pair_str(2, tag);
        }
        EntityData::AttDef {
            tag,
            prompt,
            default_value,
            insertion,
            height,
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(40, *height);
            w.pair_str(1, default_value);
            w.pair_str(2, tag);
            w.pair_str(3, prompt);
        }
        EntityData::Leader {
            vertices,
            has_arrowhead,
        } => {
            w.pair_i16(71, if *has_arrowhead { 1 } else { 0 });
            w.pair_i32(76, vertices.len() as i32);
            for v in vertices {
                w.point3d(10, *v);
            }
        }
        EntityData::MLine {
            vertices,
            style_name,
            scale,
        } => {
            w.pair_str(2, style_name);
            w.pair_f64(40, *scale);
            w.pair_i16(72, vertices.len() as i16);
            for v in vertices {
                w.point3d(11, *v);
            }
        }
        EntityData::Image {
            insertion,
            u_vector,
            v_vector,
            image_size,
        } => {
            w.point3d(10, *insertion);
            w.point3d(11, *u_vector);
            w.point3d(12, *v_vector);
            w.pair_f64(13, image_size[0]);
            w.pair_f64(23, image_size[1]);
        }
        EntityData::Wipeout { clip_vertices } => {
            w.pair_i32(91, clip_vertices.len() as i32);
            for v in clip_vertices {
                w.pair_f64(14, v[0]);
                w.pair_f64(24, v[1]);
            }
        }
        EntityData::Tolerance {
            text, insertion, ..
        } => {
            w.pair_str(1, text);
            w.point3d(10, *insertion);
        }
        EntityData::Shape {
            insertion,
            size,
            shape_number,
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(40, *size);
            w.pair_i16(2, *shape_number);
        }
        EntityData::Solid3D { acis_data } | EntityData::Region { acis_data } => {
            for line in acis_data.lines() {
                w.pair_str(1, line);
            }
        }
        EntityData::MultiLeader {} => {
            // Minimal — full MULTILEADER write is TODO
        }
        EntityData::Table {} => {}
        EntityData::Mesh {
            vertex_count,
            face_count,
        } => {
            w.pair_i32(91, *vertex_count);
            w.pair_i32(92, *face_count);
        }
        EntityData::PdfUnderlay {
            insertion, scale, ..
        } => {
            w.point3d(10, *insertion);
            w.pair_f64(41, scale[0]);
            w.pair_f64(42, scale[1]);
            w.pair_f64(43, scale[2]);
        }
        EntityData::Unknown { .. } => {
            // Cannot faithfully rewrite unknown entities
        }
    }
}

fn write_hatch_edge(w: &mut DxfWriter, edge: &HatchEdge) {
    match edge {
        HatchEdge::Line { start, end } => {
            w.pair_i16(72, 1);
            w.point2d(10, *start);
            w.point2d(11, *end);
        }
        HatchEdge::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            is_ccw,
        } => {
            w.pair_i16(72, 2);
            w.point2d(10, *center);
            w.pair_f64(40, *radius);
            w.pair_f64(50, *start_angle);
            w.pair_f64(51, *end_angle);
            w.pair_i16(73, if *is_ccw { 1 } else { 0 });
        }
        HatchEdge::EllipticArc {
            center,
            major_endpoint,
            minor_ratio,
            start_angle,
            end_angle,
            is_ccw,
        } => {
            w.pair_i16(72, 3);
            w.point2d(10, *center);
            w.point2d(11, *major_endpoint);
            w.pair_f64(40, *minor_ratio);
            w.pair_f64(50, *start_angle);
            w.pair_f64(51, *end_angle);
            w.pair_i16(73, if *is_ccw { 1 } else { 0 });
        }
        HatchEdge::Polyline { closed, vertices } => {
            w.pair_i16(72, 0);
            w.pair_i16(73, if *closed { 1 } else { 0 });
            w.pair_i32(93, vertices.len() as i32);
            for v in vertices {
                w.pair_f64(10, v[0]);
                w.pair_f64(20, v[1]);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// OBJECTS
// ---------------------------------------------------------------------------

fn write_objects(w: &mut DxfWriter, doc: &CadDocument) {
    w.pair_str(0, "SECTION");
    w.pair_str(2, "OBJECTS");

    for obj in &doc.objects {
        write_object(w, obj);
    }

    w.pair_str(0, "ENDSEC");
}

fn write_object(w: &mut DxfWriter, obj: &CadObject) {
    match &obj.data {
        ObjectData::Dictionary { entries } => {
            w.pair_str(0, "DICTIONARY");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            for (name, handle) in entries {
                w.pair_str(3, name);
                w.pair_handle(350, *handle);
            }
        }
        ObjectData::XRecord { data_pairs } => {
            w.pair_str(0, "XRECORD");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            for (code, value) in data_pairs {
                w.pair(*code, value);
            }
        }
        ObjectData::Group {
            description,
            entity_handles,
        } => {
            w.pair_str(0, "GROUP");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(300, description);
            for h in entity_handles {
                w.pair_handle(340, *h);
            }
        }
        ObjectData::Layout {
            name,
            tab_order,
            block_record_handle,
            plot_paper_size,
            plot_origin,
        } => {
            w.pair_str(0, "LAYOUT");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, name);
            w.pair_i32(71, *tab_order);
            w.pair_handle(330, *block_record_handle);
            w.pair_f64(44, plot_paper_size[0]);
            w.pair_f64(45, plot_paper_size[1]);
            w.pair_f64(46, plot_origin[0]);
            w.pair_f64(47, plot_origin[1]);
        }
        ObjectData::DictionaryVar { schema, value } => {
            w.pair_str(0, "DICTIONARYVAR");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(280, schema);
            w.pair_str(1, value);
        }
        ObjectData::Scale {
            name,
            paper_units,
            drawing_units,
            is_unit_scale,
        } => {
            w.pair_str(0, "SCALE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(300, name);
            w.pair_f64(140, *paper_units);
            w.pair_f64(141, *drawing_units);
            w.pair_i16(290, if *is_unit_scale { 1 } else { 0 });
        }
        ObjectData::VisualStyle {
            description,
            style_type,
        } => {
            w.pair_str(0, "VISUALSTYLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(2, description);
            w.pair_i32(70, *style_type);
        }
        ObjectData::Material { name } => {
            w.pair_str(0, "MATERIAL");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, name);
        }
        ObjectData::ImageDef {
            file_name,
            image_size,
        } => {
            w.pair_str(0, "IMAGEDEF");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, file_name);
            w.pair_f64(10, image_size[0]);
            w.pair_f64(20, image_size[1]);
        }
        ObjectData::ImageDefReactor { image_handle } => {
            w.pair_str(0, "IMAGEDEF_REACTOR");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_handle(330, *image_handle);
        }
        ObjectData::MLineStyle {
            name,
            description,
            element_count,
        } => {
            w.pair_str(0, "MLINESTYLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(2, name);
            w.pair_str(3, description);
            w.pair_i16(71, *element_count);
        }
        ObjectData::MLeaderStyle {
            name,
            content_type,
            text_style_handle,
        } => {
            w.pair_str(0, "MLEADERSTYLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(2, name);
            w.pair_i16(170, *content_type);
            w.pair_handle(340, *text_style_handle);
        }
        ObjectData::TableStyle {
            name, description, ..
        } => {
            w.pair_str(0, "TABLESTYLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(2, name);
            w.pair_str(3, description);
        }
        ObjectData::SortEntsTable {
            entity_handles,
            sort_handles,
        } => {
            w.pair_str(0, "SORTENTSTABLE");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            for (eh, sh) in entity_handles.iter().zip(sort_handles.iter()) {
                w.pair_handle(331, *eh);
                w.pair_handle(5, *sh);
            }
        }
        ObjectData::DimAssoc {
            associativity,
            dimension_handle,
        } => {
            w.pair_str(0, "DIMASSOC");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_i32(90, *associativity);
            w.pair_handle(330, *dimension_handle);
        }
        ObjectData::PlotSettings {
            page_name,
            printer_name,
            paper_size,
        } => {
            w.pair_str(0, "PLOTSETTINGS");
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
            w.pair_str(1, page_name);
            w.pair_str(2, printer_name);
            w.pair_str(4, paper_size);
        }
        ObjectData::Unknown { object_type } => {
            w.pair_str(0, object_type);
            w.pair_handle(5, obj.handle);
            w.pair_handle(330, obj.owner_handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_f64_trims_trailing_zeros() {
        assert_eq!(format_f64(0.0), "0.0");
        assert_eq!(format_f64(1.0), "1.0");
        assert_eq!(format_f64(1.5), "1.5");
        assert_eq!(format_f64(1.25), "1.25");
        assert_eq!(format_f64(-3.14), "-3.14");
    }

    #[test]
    fn write_minimal_document() {
        let doc = CadDocument::new();
        let output = write_dxf_string(&doc).unwrap();
        assert!(output.contains("SECTION"));
        assert!(output.contains("HEADER"));
        assert!(output.contains("$ACADVER"));
        assert!(output.contains("AC1015"));
        assert!(output.contains("ENDSEC"));
        assert!(output.contains("EOF"));
        assert!(output.contains("BLOCK_RECORD"));
        assert!(output.contains("*Model_Space"));
        assert!(output.contains("*Paper_Space"));
    }
}
