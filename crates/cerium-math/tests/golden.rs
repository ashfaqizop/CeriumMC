//! Replays `Mth` calls recorded from the vanilla server (see `tools/cerium-golden`).

use cerium_golden::{Case, Report, Value, load};
use cerium_math::mth;

#[test]
fn mth_matches_vanilla() {
    let Some(cases) = load("mth.txt") else {
        return;
    };
    let mut report = Report::new("mth.txt");
    for case in &cases {
        report.check(case, &eval(case));
    }
    report.finish();
}

fn index(case: &Case) -> usize {
    usize::try_from(case.int(0)).expect("table index is non-negative")
}

#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "one arm per vanilla function, with one-letter argument accessors"
)]
fn eval(c: &Case) -> Vec<Value> {
    use Value::{Bool, Byte, Double, Float, Int, Long};

    let f = |k| c.float(k);
    let d = |k| c.double(k);
    let i = |k| c.int(k);
    let l = |k| c.long(k);

    let value = match c.op.as_str() {
        "sintab" => Float(mth::sin_table()[index(c)]),
        "asintab" => Double(mth::asin_table()[index(c)]),
        "costab" => Double(mth::cos_table()[index(c)]),
        "const" => Float(match c.string(0).as_str() {
            "PI" => mth::PI,
            "HALF_PI" => mth::HALF_PI,
            "TWO_PI" => mth::TWO_PI,
            "DEG_TO_RAD" => mth::DEG_TO_RAD,
            "RAD_TO_DEG" => mth::RAD_TO_DEG,
            "SQRT_OF_TWO" => mth::SQRT_OF_TWO,
            other => panic!("line {}: unknown constant {other}", c.line),
        }),
        "sin" => Float(mth::sin(d(0))),
        "cos" => Float(mth::cos(d(0))),
        "sqrt" => Float(mth::sqrt(f(0))),
        "floor_f32" => Int(mth::floor_f32(f(0))),
        "floor_f64" => Int(mth::floor_f64(d(0))),
        "lfloor" => Long(mth::lfloor(d(0))),
        "abs_f32" => Float(mth::abs_f32(f(0))),
        "abs_i32" => Int(mth::abs_i32(i(0))),
        "ceil_f32" => Int(mth::ceil_f32(f(0))),
        "ceil_f64" => Int(mth::ceil_f64(d(0))),
        "ceil_long" => Long(mth::ceil_long(d(0))),
        "clamp_i32" => Int(mth::clamp_i32(i(0), i(1), i(2))),
        "clamp_i64" => Long(mth::clamp_i64(l(0), l(1), l(2))),
        "clamp_f32" => Float(mth::clamp_f32(f(0), f(1), f(2))),
        "clamp_f64" => Double(mth::clamp_f64(d(0), d(1), d(2))),
        "clamped_lerp_f64" => Double(mth::clamped_lerp_f64(d(0), d(1), d(2))),
        "clamped_lerp_f32" => Float(mth::clamped_lerp_f32(f(0), f(1), f(2))),
        "abs_max_i32" => Int(mth::abs_max_i32(i(0), i(1))),
        "abs_max_f32" => Float(mth::abs_max_f32(f(0), f(1))),
        "abs_max_f64" => Double(mth::abs_max_f64(d(0), d(1))),
        "chessboard_distance" => Int(mth::chessboard_distance(i(0), i(1), i(2), i(3))),
        "floor_div" => Int(mth::floor_div(i(0), i(1))),
        "equal_f32" => Bool(mth::equal_f32(f(0), f(1))),
        "equal_f64" => Bool(mth::equal_f64(d(0), d(1))),
        "positive_modulo_i32" => Int(mth::positive_modulo_i32(i(0), i(1))),
        "positive_modulo_f32" => Float(mth::positive_modulo_f32(f(0), f(1))),
        "positive_modulo_f64" => Double(mth::positive_modulo_f64(d(0), d(1))),
        "is_multiple_of" => Bool(mth::is_multiple_of(i(0), i(1))),
        "pack_degrees" => Byte(mth::pack_degrees(f(0))),
        "unpack_degrees" => Float(mth::unpack_degrees(c.byte(0))),
        "wrap_degrees_i32" => Int(mth::wrap_degrees_i32(i(0))),
        "wrap_degrees_i64" => Float(mth::wrap_degrees_i64(l(0))),
        "wrap_degrees_f32" => Float(mth::wrap_degrees_f32(f(0))),
        "wrap_degrees_f64" => Double(mth::wrap_degrees_f64(d(0))),
        "wrap_degrees_90" => Float(mth::wrap_degrees_90(f(0))),
        "degrees_difference" => Float(mth::degrees_difference(f(0), f(1))),
        "degrees_difference_abs" => Float(mth::degrees_difference_abs(f(0), f(1))),
        "rotate_if_necessary" => Float(mth::rotate_if_necessary(f(0), f(1), f(2))),
        "approach" => Float(mth::approach(f(0), f(1), f(2))),
        "approach_degrees" => Float(mth::approach_degrees(f(0), f(1), f(2))),
        "smallest_encompassing_power_of_two" => Int(mth::smallest_encompassing_power_of_two(i(0))),
        "smallest_square_side" => Int(mth::smallest_square_side(i(0))),
        "is_power_of_two_i32" => Bool(mth::is_power_of_two_i32(i(0))),
        "is_power_of_two_i64" => Bool(mth::is_power_of_two_i64(l(0))),
        "ceillog2" => Int(mth::ceillog2(i(0))),
        "log2" => Int(mth::log2(i(0))),
        "frac_f32" => Float(mth::frac_f32(f(0))),
        "frac_f64" => Double(mth::frac_f64(d(0))),
        "get_seed" => Long(mth::get_seed(i(0), i(1), i(2))),
        "inverse_lerp_f64" => Double(mth::inverse_lerp_f64(d(0), d(1), d(2))),
        "inverse_lerp_f32" => Float(mth::inverse_lerp_f32(f(0), f(1), f(2))),
        "atan2" => Double(mth::atan2(d(0), d(1))),
        "inv_sqrt_f32" => Float(mth::inv_sqrt_f32(f(0))),
        "inv_sqrt_f64" => Double(mth::inv_sqrt_f64(d(0))),
        "fast_inv_sqrt" => Double(mth::fast_inv_sqrt(d(0))),
        "fast_inv_cube_root" => Float(mth::fast_inv_cube_root(f(0))),
        "hsv_to_argb" => Int(mth::hsv_to_argb(f(0), f(1), f(2), i(3))),
        "murmur_hash3_mixer" => Int(mth::murmur_hash3_mixer(i(0))),
        "binary_search" => {
            let threshold = i(2);
            Int(mth::binary_search(i(0), i(1), |x| x >= threshold))
        }
        "lerp_int" => Int(mth::lerp_int(f(0), i(1), i(2))),
        "lerp_discrete" => Int(mth::lerp_discrete(f(0), i(1), i(2))),
        "lerp_f32" => Float(mth::lerp_f32(f(0), f(1), f(2))),
        "lerp_f64" => Double(mth::lerp_f64(d(0), d(1), d(2))),
        "lerp2_f32" => Float(mth::lerp2_f32(f(0), f(1), f(2), f(3), f(4), f(5))),
        "lerp2_f64" => Double(mth::lerp2_f64(d(0), d(1), d(2), d(3), d(4), d(5))),
        "lerp3_f32" => Float(mth::lerp3_f32(
            f(0),
            f(1),
            f(2),
            f(3),
            f(4),
            f(5),
            f(6),
            f(7),
            f(8),
            f(9),
            f(10),
        )),
        "lerp3_f64" => Double(mth::lerp3_f64(
            d(0),
            d(1),
            d(2),
            d(3),
            d(4),
            d(5),
            d(6),
            d(7),
            d(8),
            d(9),
            d(10),
        )),
        "catmullrom" => Float(mth::catmullrom(f(0), f(1), f(2), f(3), f(4))),
        "smoothstep" => Float(mth::smoothstep(f(0))),
        "smoothstep_derivative" => Float(mth::smoothstep_derivative(f(0))),
        "sign" => Int(mth::sign(d(0))),
        "rot_lerp_f32" => Float(mth::rot_lerp_f32(f(0), f(1), f(2))),
        "rot_lerp_f64" => Double(mth::rot_lerp_f64(d(0), d(1), d(2))),
        "rot_lerp_rad" => Float(mth::rot_lerp_rad(f(0), f(1), f(2))),
        "triangle_wave" => Float(mth::triangle_wave(f(0), f(1))),
        "square_f32" => Float(mth::square_f32(f(0))),
        "cube" => Float(mth::cube(f(0))),
        "square_f64" => Double(mth::square_f64(d(0))),
        "square_i32" => Int(mth::square_i32(i(0))),
        "square_i64" => Long(mth::square_i64(l(0))),
        "clamped_map_f64" => Double(mth::clamped_map_f64(d(0), d(1), d(2), d(3), d(4))),
        "clamped_map_f32" => Float(mth::clamped_map_f32(f(0), f(1), f(2), f(3), f(4))),
        "map_f64" => Double(mth::map_f64(d(0), d(1), d(2), d(3), d(4))),
        "map_f32" => Float(mth::map_f32(f(0), f(1), f(2), f(3), f(4))),
        "round_toward_i32" => Int(mth::round_toward_i32(i(0), i(1))),
        "round_toward_i64" => Long(mth::round_toward_i64(l(0), l(1))),
        "positive_ceil_div_i32" => Int(mth::positive_ceil_div_i32(i(0), i(1))),
        "positive_ceil_div_i64" => Long(mth::positive_ceil_div_i64(l(0), l(1))),
        "length_squared_2d_f64" => Double(mth::length_squared_2d_f64(d(0), d(1))),
        "length_squared_2d_f32" => Float(mth::length_squared_2d_f32(f(0), f(1))),
        "length_2d_f64" => Double(mth::length_2d_f64(d(0), d(1))),
        "length_2d_f32" => Float(mth::length_2d_f32(f(0), f(1))),
        "length_squared_3d_f64" => Double(mth::length_squared_3d_f64(d(0), d(1), d(2))),
        "length_3d_f64" => Double(mth::length_3d_f64(d(0), d(1), d(2))),
        "length_3d_f32" => Float(mth::length_3d_f32(f(0), f(1), f(2))),
        "length_squared_3d_f32" => Float(mth::length_squared_3d_f32(f(0), f(1), f(2))),
        "quantize" => Int(mth::quantize(d(0), i(1))),
        "out_from_origin" => {
            return mth::out_from_origin(i(0), i(1), i(2), i(3))
                .map(Int)
                .collect();
        }
        other => panic!("line {}: unknown op {other}", c.line),
    };
    vec![value]
}
