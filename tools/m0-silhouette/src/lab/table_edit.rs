//! Field catalog, path get/set, and RON emission for experiment tables.
//!
//! The product table only deserializes ([`Table::from_ron`](treepo_gen::Table::from_ron)).
//! The lab mutates a live [`Table`] and writes a comment-free RON that still validates under
//! the same loader.

use treepo_gen::Table;
use treepo_gen::params::{GroundTable, Row, Scales, TrunkTable, Weights};

/// One tunable integer field exposed in the UI.
#[derive(Debug, Clone)]
pub(super) struct FieldMeta {
    /// Dot path, e.g. `width_ratio.base` or `trunk.basal_aspect`.
    pub(super) path: &'static str,
    /// §5 / findings family label.
    pub(super) family: &'static str,
    /// Human unit hint.
    pub(super) unit: &'static str,
    /// Soft slider floor (may be wider than validate for weights).
    pub(super) soft_min: i32,
    /// Soft slider ceiling.
    pub(super) soft_max: i32,
    /// Slider step.
    pub(super) step: i32,
}

/// Every field the lab can focus, one family at a time.
pub(super) fn catalog() -> &'static [FieldMeta] {
    &CATALOG
}

/// Read an integer at `path`.
pub(super) fn get_value(table: &Table, path: &str) -> Result<i32, String> {
    match path {
        "scales.depth_full_scale" => Ok(i32::from(table.scales.depth_full_scale)),
        "scales.bushiness_full_scale" => Ok(i32::from(table.scales.bushiness_full_scale)),
        "scales.diversity_full_scale" => Ok(i32::from(table.scales.diversity_full_scale)),
        "scales.fragmentation_full_scale" => Ok(i32::from(table.scales.fragmentation_full_scale)),
        other => Ok(*resolve(table, other)?),
    }
}

/// Write an integer at `path` (no validate — prefer [`set_value_validated`]).
pub(super) fn set_value(table: &mut Table, path: &str, value: i32) -> Result<(), String> {
    match path {
        "scales.depth_full_scale" => {
            table.scales.depth_full_scale = u16_from_i32(value, path)?;
        }
        "scales.bushiness_full_scale" => {
            table.scales.bushiness_full_scale = u16_from_i32(value, path)?;
        }
        "scales.diversity_full_scale" => {
            table.scales.diversity_full_scale = u16_from_i32(value, path)?;
        }
        "scales.fragmentation_full_scale" => {
            table.scales.fragmentation_full_scale = u16_from_i32(value, path)?;
        }
        other => {
            *resolve_mut(table, other)? = value;
        }
    }
    Ok(())
}

/// Apply a value, re-validate, and roll back on failure.
pub(super) fn set_value_validated(
    table: &mut Table,
    path: &str,
    value: i32,
) -> Result<i32, String> {
    let previous = get_value(table, path)?;
    set_value(table, path, value)?;
    if let Err(error) = table.validate() {
        set_value(table, path, previous).expect("rollback path must still resolve");
        return Err(error.to_string());
    }
    Ok(previous)
}

/// Emit a compact RON document that `Table::from_ron` accepts.
pub(super) fn to_ron(table: &Table) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("(\n");
    out.push_str(&format!("    version: {},\n", table.version));
    out.push_str("    scales: (\n");
    write_scales(&mut out, &table.scales);
    out.push_str("    ),\n");
    out.push_str(&format!("    min_length: {},\n", table.min_length));
    write_row(&mut out, "recursion", &table.recursion);
    write_row(&mut out, "branch_angle", &table.branch_angle);
    write_row(&mut out, "angle_jitter", &table.angle_jitter);
    write_row(&mut out, "length_jitter", &table.length_jitter);
    write_row(&mut out, "length_ratio", &table.length_ratio);
    write_row(&mut out, "width_ratio", &table.width_ratio);
    write_row(&mut out, "droop", &table.droop);
    write_row(&mut out, "tropism", &table.tropism);
    out.push_str("    ground: (\n");
    write_ground(&mut out, &table.ground);
    out.push_str("    ),\n");
    write_row(&mut out, "base_length", &table.base_length);
    write_row(&mut out, "base_width", &table.base_width);
    write_row(&mut out, "branch_capacity", &table.branch_capacity);
    out.push_str("    trunk: (\n");
    write_trunk(&mut out, &table.trunk);
    out.push_str("    ),\n");
    out.push_str(")\n");
    out
}

/// Diff summary of changed catalog paths vs a baseline table.
pub(super) fn diff_summary(baseline: &Table, current: &Table) -> Vec<(String, i32, i32)> {
    let mut changes = Vec::new();
    for field in catalog() {
        let from = get_value(baseline, field.path).unwrap_or(0);
        let to = get_value(current, field.path).unwrap_or(0);
        if from != to {
            changes.push((field.path.to_owned(), from, to));
        }
    }
    changes
}

fn write_scales(out: &mut String, s: &Scales) {
    out.push_str(&format!(
        "        depth_full_scale: {},\n        bushiness_full_scale: {},\n        \
         diversity_full_scale: {},\n        fragmentation_full_scale: {},\n",
        s.depth_full_scale,
        s.bushiness_full_scale,
        s.diversity_full_scale,
        s.fragmentation_full_scale
    ));
}

fn write_ground(out: &mut String, g: &GroundTable) {
    out.push_str(&format!(
        "        engage: {},\n        release: {},\n        lift: {},\n",
        g.engage, g.release, g.lift
    ));
}

fn write_trunk(out: &mut String, t: &TrunkTable) {
    out.push_str(&format!("        basal_aspect: {},\n", t.basal_aspect));
    out.push_str(&format!("        basal_min: {},\n", t.basal_min));
    out.push_str(&format!("        flare: {},\n", t.flare));
    out.push_str(&format!(
        "        internode_aspect: {},\n",
        t.internode_aspect
    ));
    out.push_str(&format!("        internode_min: {},\n", t.internode_min));
    out.push_str(&format!("        packing: {},\n", t.packing));
    out.push_str(&format!("        support_knee: {},\n", t.support_knee));
    out.push_str(&format!("        support_beyond: {},\n", t.support_beyond));
    write_row_nested(out, "fan", &t.fan, 2);
    write_row_nested(out, "root_cluster", &t.root_cluster, 2);
    out.push_str(&format!("        group_below: {},\n", t.group_below));
}

fn write_row(out: &mut String, name: &str, row: &Row) {
    write_row_nested(out, name, row, 1);
}

fn write_row_nested(out: &mut String, name: &str, row: &Row, indent_level: usize) {
    let pad = "    ".repeat(indent_level);
    let inner = "    ".repeat(indent_level + 1);
    out.push_str(&format!(
        "{pad}{name}: (base: {}, min: {}, max: {}, per: (\n",
        row.base, row.min, row.max
    ));
    write_weights(out, &row.per, &inner);
    out.push_str(&format!("{pad})),\n"));
}

fn write_weights(out: &mut String, w: &Weights, pad: &str) {
    // Only emit non-zero weights so the experiment file stays short.
    let pairs = [
        ("mass", w.mass),
        ("depth", w.depth),
        ("bushiness", w.bushiness),
        ("skew", w.skew),
        ("skew_abs", w.skew_abs),
        ("balance", w.balance),
        ("convention", w.convention),
        ("diversity", w.diversity),
        ("fragmentation", w.fragmentation),
    ];
    for (name, value) in pairs {
        if value != 0 {
            out.push_str(&format!("{pad}{name}: {value},\n"));
        }
    }
}

macro_rules! row_fields {
    ($row:expr, $rest:expr) => {
        match $rest {
            "base" => Ok(&$row.base),
            "min" => Ok(&$row.min),
            "max" => Ok(&$row.max),
            "per.mass" => Ok(&$row.per.mass),
            "per.depth" => Ok(&$row.per.depth),
            "per.bushiness" => Ok(&$row.per.bushiness),
            "per.skew" => Ok(&$row.per.skew),
            "per.skew_abs" => Ok(&$row.per.skew_abs),
            "per.balance" => Ok(&$row.per.balance),
            "per.convention" => Ok(&$row.per.convention),
            "per.diversity" => Ok(&$row.per.diversity),
            "per.fragmentation" => Ok(&$row.per.fragmentation),
            other => Err(format!("unknown row field `{other}`")),
        }
    };
}

macro_rules! row_fields_mut {
    ($row:expr, $rest:expr) => {
        match $rest {
            "base" => Ok(&mut $row.base),
            "min" => Ok(&mut $row.min),
            "max" => Ok(&mut $row.max),
            "per.mass" => Ok(&mut $row.per.mass),
            "per.depth" => Ok(&mut $row.per.depth),
            "per.bushiness" => Ok(&mut $row.per.bushiness),
            "per.skew" => Ok(&mut $row.per.skew),
            "per.skew_abs" => Ok(&mut $row.per.skew_abs),
            "per.balance" => Ok(&mut $row.per.balance),
            "per.convention" => Ok(&mut $row.per.convention),
            "per.diversity" => Ok(&mut $row.per.diversity),
            "per.fragmentation" => Ok(&mut $row.per.fragmentation),
            other => Err(format!("unknown row field `{other}`")),
        }
    };
}

fn resolve<'a>(table: &'a Table, path: &str) -> Result<&'a i32, String> {
    match path {
        "min_length" => Ok(&table.min_length),
        "ground.engage" => Ok(&table.ground.engage),
        "ground.release" => Ok(&table.ground.release),
        "ground.lift" => Ok(&table.ground.lift),
        "trunk.basal_aspect" => Ok(&table.trunk.basal_aspect),
        "trunk.basal_min" => Ok(&table.trunk.basal_min),
        "trunk.flare" => Ok(&table.trunk.flare),
        "trunk.internode_aspect" => Ok(&table.trunk.internode_aspect),
        "trunk.internode_min" => Ok(&table.trunk.internode_min),
        "trunk.packing" => Ok(&table.trunk.packing),
        "trunk.support_knee" => Ok(&table.trunk.support_knee),
        "trunk.support_beyond" => Ok(&table.trunk.support_beyond),
        "trunk.group_below" => Ok(&table.trunk.group_below),
        p if p.starts_with("recursion.") => row_fields!(table.recursion, &p["recursion.".len()..]),
        p if p.starts_with("branch_angle.") => {
            row_fields!(table.branch_angle, &p["branch_angle.".len()..])
        }
        p if p.starts_with("angle_jitter.") => {
            row_fields!(table.angle_jitter, &p["angle_jitter.".len()..])
        }
        p if p.starts_with("length_jitter.") => {
            row_fields!(table.length_jitter, &p["length_jitter.".len()..])
        }
        p if p.starts_with("length_ratio.") => {
            row_fields!(table.length_ratio, &p["length_ratio.".len()..])
        }
        p if p.starts_with("width_ratio.") => {
            row_fields!(table.width_ratio, &p["width_ratio.".len()..])
        }
        p if p.starts_with("droop.") => row_fields!(table.droop, &p["droop.".len()..]),
        p if p.starts_with("tropism.") => row_fields!(table.tropism, &p["tropism.".len()..]),
        p if p.starts_with("base_length.") => {
            row_fields!(table.base_length, &p["base_length.".len()..])
        }
        p if p.starts_with("base_width.") => {
            row_fields!(table.base_width, &p["base_width.".len()..])
        }
        p if p.starts_with("branch_capacity.") => {
            row_fields!(table.branch_capacity, &p["branch_capacity.".len()..])
        }
        p if p.starts_with("trunk.fan.") => row_fields!(table.trunk.fan, &p["trunk.fan.".len()..]),
        p if p.starts_with("trunk.root_cluster.") => {
            row_fields!(table.trunk.root_cluster, &p["trunk.root_cluster.".len()..])
        }
        other => Err(format!("unknown parameter path `{other}`")),
    }
}

fn resolve_mut<'a>(table: &'a mut Table, path: &str) -> Result<&'a mut i32, String> {
    match path {
        "min_length" => Ok(&mut table.min_length),
        "ground.engage" => Ok(&mut table.ground.engage),
        "ground.release" => Ok(&mut table.ground.release),
        "ground.lift" => Ok(&mut table.ground.lift),
        "trunk.basal_aspect" => Ok(&mut table.trunk.basal_aspect),
        "trunk.basal_min" => Ok(&mut table.trunk.basal_min),
        "trunk.flare" => Ok(&mut table.trunk.flare),
        "trunk.internode_aspect" => Ok(&mut table.trunk.internode_aspect),
        "trunk.internode_min" => Ok(&mut table.trunk.internode_min),
        "trunk.packing" => Ok(&mut table.trunk.packing),
        "trunk.support_knee" => Ok(&mut table.trunk.support_knee),
        "trunk.support_beyond" => Ok(&mut table.trunk.support_beyond),
        "trunk.group_below" => Ok(&mut table.trunk.group_below),
        p if p.starts_with("recursion.") => {
            row_fields_mut!(table.recursion, &p["recursion.".len()..])
        }
        p if p.starts_with("branch_angle.") => {
            row_fields_mut!(table.branch_angle, &p["branch_angle.".len()..])
        }
        p if p.starts_with("angle_jitter.") => {
            row_fields_mut!(table.angle_jitter, &p["angle_jitter.".len()..])
        }
        p if p.starts_with("length_jitter.") => {
            row_fields_mut!(table.length_jitter, &p["length_jitter.".len()..])
        }
        p if p.starts_with("length_ratio.") => {
            row_fields_mut!(table.length_ratio, &p["length_ratio.".len()..])
        }
        p if p.starts_with("width_ratio.") => {
            row_fields_mut!(table.width_ratio, &p["width_ratio.".len()..])
        }
        p if p.starts_with("droop.") => row_fields_mut!(table.droop, &p["droop.".len()..]),
        p if p.starts_with("tropism.") => row_fields_mut!(table.tropism, &p["tropism.".len()..]),
        p if p.starts_with("base_length.") => {
            row_fields_mut!(table.base_length, &p["base_length.".len()..])
        }
        p if p.starts_with("base_width.") => {
            row_fields_mut!(table.base_width, &p["base_width.".len()..])
        }
        p if p.starts_with("branch_capacity.") => {
            row_fields_mut!(table.branch_capacity, &p["branch_capacity.".len()..])
        }
        p if p.starts_with("trunk.fan.") => {
            row_fields_mut!(table.trunk.fan, &p["trunk.fan.".len()..])
        }
        p if p.starts_with("trunk.root_cluster.") => {
            row_fields_mut!(table.trunk.root_cluster, &p["trunk.root_cluster.".len()..])
        }
        other => Err(format!("unknown parameter path `{other}`")),
    }
}

fn u16_from_i32(value: i32, path: &str) -> Result<u16, String> {
    u16::try_from(value).map_err(|_| format!("{path} must fit in u16, got {value}"))
}

// Soft ranges are for the slider; validate still owns the real gate.
const CATALOG: [FieldMeta; 48] = [
    FieldMeta {
        path: "recursion.base",
        family: "A",
        unit: "milli-levels",
        soft_min: 1000,
        soft_max: 5000,
        step: 50,
    },
    FieldMeta {
        path: "recursion.min",
        family: "A",
        unit: "milli-levels",
        soft_min: 1000,
        soft_max: 5000,
        step: 50,
    },
    FieldMeta {
        path: "recursion.max",
        family: "A",
        unit: "milli-levels",
        soft_min: 1000,
        soft_max: 5000,
        step: 50,
    },
    FieldMeta {
        path: "branch_angle.base",
        family: "B",
        unit: "millidegrees",
        soft_min: 15000,
        soft_max: 60000,
        step: 500,
    },
    FieldMeta {
        path: "branch_angle.min",
        family: "B",
        unit: "millidegrees",
        soft_min: 15000,
        soft_max: 60000,
        step: 500,
    },
    FieldMeta {
        path: "branch_angle.max",
        family: "B",
        unit: "millidegrees",
        soft_min: 15000,
        soft_max: 60000,
        step: 500,
    },
    FieldMeta {
        path: "length_ratio.base",
        family: "C",
        unit: "per mille",
        soft_min: 400,
        soft_max: 900,
        step: 5,
    },
    FieldMeta {
        path: "length_ratio.min",
        family: "C",
        unit: "per mille",
        soft_min: 400,
        soft_max: 900,
        step: 5,
    },
    FieldMeta {
        path: "length_ratio.max",
        family: "C",
        unit: "per mille",
        soft_min: 400,
        soft_max: 900,
        step: 5,
    },
    FieldMeta {
        path: "width_ratio.base",
        family: "C",
        unit: "per mille",
        soft_min: 700,
        soft_max: 990,
        step: 5,
    },
    FieldMeta {
        path: "width_ratio.min",
        family: "C",
        unit: "per mille",
        soft_min: 700,
        soft_max: 990,
        step: 5,
    },
    FieldMeta {
        path: "width_ratio.max",
        family: "C",
        unit: "per mille",
        soft_min: 700,
        soft_max: 990,
        step: 5,
    },
    FieldMeta {
        path: "angle_jitter.base",
        family: "D",
        unit: "millidegrees",
        soft_min: 0,
        soft_max: 24000,
        step: 250,
    },
    FieldMeta {
        path: "angle_jitter.max",
        family: "D",
        unit: "millidegrees",
        soft_min: 0,
        soft_max: 24000,
        step: 250,
    },
    FieldMeta {
        path: "length_jitter.base",
        family: "D",
        unit: "per mille",
        soft_min: 0,
        soft_max: 400,
        step: 5,
    },
    FieldMeta {
        path: "length_jitter.max",
        family: "D",
        unit: "per mille",
        soft_min: 0,
        soft_max: 400,
        step: 5,
    },
    FieldMeta {
        path: "droop.base",
        family: "E",
        unit: "millidegrees",
        soft_min: 0,
        soft_max: 18000,
        step: 250,
    },
    FieldMeta {
        path: "droop.max",
        family: "E",
        unit: "millidegrees",
        soft_min: 0,
        soft_max: 18000,
        step: 250,
    },
    FieldMeta {
        path: "droop.per.mass",
        family: "E",
        unit: "millidegrees",
        soft_min: 0,
        soft_max: 30000,
        step: 250,
    },
    FieldMeta {
        path: "tropism.base",
        family: "Tropism",
        unit: "millidegrees",
        soft_min: 0,
        soft_max: 45000,
        step: 250,
    },
    FieldMeta {
        path: "tropism.max",
        family: "Tropism",
        unit: "millidegrees",
        soft_min: 0,
        soft_max: 45000,
        step: 250,
    },
    FieldMeta {
        path: "ground.engage",
        family: "Tropism",
        unit: "millidegrees",
        soft_min: 30000,
        soft_max: 120000,
        step: 1000,
    },
    FieldMeta {
        path: "ground.release",
        family: "Tropism",
        unit: "millidegrees",
        soft_min: 0,
        soft_max: 120000,
        step: 1000,
    },
    FieldMeta {
        path: "ground.lift",
        family: "Tropism",
        unit: "millidegrees",
        soft_min: 0,
        soft_max: 45000,
        step: 250,
    },
    FieldMeta {
        path: "base_length.base",
        family: "Base",
        unit: "per mille",
        soft_min: 100,
        soft_max: 4000,
        step: 25,
    },
    FieldMeta {
        path: "base_width.base",
        family: "Base",
        unit: "per mille",
        soft_min: 20,
        soft_max: 1000,
        step: 10,
    },
    FieldMeta {
        path: "min_length",
        family: "Base",
        unit: "per mille",
        soft_min: 10,
        soft_max: 500,
        step: 5,
    },
    FieldMeta {
        path: "branch_capacity.base",
        family: "Capacity",
        unit: "children",
        soft_min: 2,
        soft_max: 32,
        step: 1,
    },
    FieldMeta {
        path: "branch_capacity.max",
        family: "Capacity",
        unit: "children",
        soft_min: 2,
        soft_max: 32,
        step: 1,
    },
    FieldMeta {
        path: "trunk.basal_aspect",
        family: "Trunk",
        unit: "per mille",
        soft_min: 200,
        soft_max: 2000,
        step: 25,
    },
    FieldMeta {
        path: "trunk.basal_min",
        family: "Trunk",
        unit: "per mille",
        soft_min: 20,
        soft_max: 400,
        step: 5,
    },
    FieldMeta {
        path: "trunk.flare",
        family: "Trunk",
        unit: "per mille",
        soft_min: 1000,
        soft_max: 2000,
        step: 10,
    },
    FieldMeta {
        path: "trunk.internode_aspect",
        family: "Trunk",
        unit: "per mille",
        soft_min: 200,
        soft_max: 4000,
        step: 25,
    },
    FieldMeta {
        path: "trunk.internode_min",
        family: "Trunk",
        unit: "per mille",
        soft_min: 20,
        soft_max: 500,
        step: 5,
    },
    FieldMeta {
        path: "trunk.packing",
        family: "Trunk",
        unit: "per mille",
        soft_min: 200,
        soft_max: 1000,
        step: 10,
    },
    FieldMeta {
        path: "trunk.support_knee",
        family: "Trunk",
        unit: "per mille",
        soft_min: 200,
        soft_max: 4000,
        step: 25,
    },
    FieldMeta {
        path: "trunk.support_beyond",
        family: "Trunk",
        unit: "per mille",
        soft_min: 50,
        soft_max: 1000,
        step: 10,
    },
    FieldMeta {
        path: "trunk.fan.base",
        family: "Trunk",
        unit: "millidegrees",
        soft_min: 20000,
        soft_max: 160000,
        step: 1000,
    },
    FieldMeta {
        path: "trunk.fan.min",
        family: "Trunk",
        unit: "millidegrees",
        soft_min: 10000,
        soft_max: 160000,
        step: 1000,
    },
    FieldMeta {
        path: "trunk.fan.max",
        family: "Trunk",
        unit: "millidegrees",
        soft_min: 20000,
        soft_max: 180000,
        step: 1000,
    },
    FieldMeta {
        path: "trunk.root_cluster.base",
        family: "Trunk",
        unit: "nodes",
        soft_min: 1,
        soft_max: 12,
        step: 1,
    },
    FieldMeta {
        path: "trunk.group_below",
        family: "Trunk",
        unit: "per mille",
        soft_min: 0,
        soft_max: 500,
        step: 5,
    },
    FieldMeta {
        path: "scales.depth_full_scale",
        family: "Scales",
        unit: "levels",
        soft_min: 2,
        soft_max: 40,
        step: 1,
    },
    FieldMeta {
        path: "scales.bushiness_full_scale",
        family: "Scales",
        unit: "children",
        soft_min: 2,
        soft_max: 20,
        step: 1,
    },
    FieldMeta {
        path: "scales.diversity_full_scale",
        family: "Scales",
        unit: "languages",
        soft_min: 2,
        soft_max: 20,
        step: 1,
    },
    FieldMeta {
        path: "scales.fragmentation_full_scale",
        family: "Scales",
        unit: "contributors",
        soft_min: 2,
        soft_max: 20,
        step: 1,
    },
    FieldMeta {
        path: "tropism.per.convention",
        family: "Tropism",
        unit: "millidegrees",
        soft_min: -20000,
        soft_max: 20000,
        step: 250,
    },
    FieldMeta {
        path: "tropism.per.bushiness",
        family: "Tropism",
        unit: "millidegrees",
        soft_min: -20000,
        soft_max: 20000,
        step: 250,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_built_in() {
        let table = Table::built_in();
        let ron = to_ron(&table);
        let again = Table::from_ron(&ron).expect("lab RON must validate");
        assert_eq!(table, again);
    }

    #[test]
    fn set_width_ratio_base() {
        let mut table = Table::built_in();
        let before = get_value(&table, "width_ratio.base").unwrap();
        set_value_validated(&mut table, "width_ratio.base", before - 10).unwrap();
        assert_eq!(get_value(&table, "width_ratio.base").unwrap(), before - 10);
    }

    #[test]
    fn invalid_set_rolls_back() {
        let mut table = Table::built_in();
        let before = get_value(&table, "width_ratio.min").unwrap();
        let err = set_value_validated(&mut table, "width_ratio.min", 999).unwrap_err();
        assert!(!err.is_empty());
        assert_eq!(get_value(&table, "width_ratio.min").unwrap(), before);
    }
}
