use mc_nbt_scanner::nbt_is_subset;
use valence_nbt::Value;
use valence_nbt::snbt::from_snbt_str;

fn parse(s: &str) -> Value {
    from_snbt_str(s).expect("Failed to parse SNBT")
}

#[test]
fn simple_compound_subset() {
    let sup = parse("{a:1, b:2, c:3}");
    let sub = parse("{a:1, c:3}");
    assert!(nbt_is_subset(&sup, &sub));
}

#[test]
fn compound_missing_key_should_fail() {
    let sup = parse("{a:1, b:2}");
    let sub = parse("{a:1, c:3}");
    assert!(!nbt_is_subset(&sup, &sub));
}

#[test]
fn unordered_list_subset() {
    let sup = parse("[1, 2, 3, 4]");
    let sub = parse("[4, 2]");
    assert!(nbt_is_subset(&sup, &sub));
}

#[test]
fn list_insufficient_elements_should_fail() {
    let sup = parse("[1, 2, 2]");
    let sub = parse("[2, 2, 2]");
    assert!(!nbt_is_subset(&sup, &sub));
}

#[test]
fn nested_structures_subset() {
    let sup = parse("{x:{y:[{z:1}, {z:2}]}, w:5}");
    let sub = parse("{x:{y:[{z:2}]}}");
    assert!(nbt_is_subset(&sup, &sub));
}

#[test]
fn primitive_equality_match_and_mismatch() {
    let sup = parse("123");
    let sub = parse("123");
    assert!(nbt_is_subset(&sup, &sub));

    let sup2 = parse("123");
    let sub2 = parse("456");
    assert!(!nbt_is_subset(&sup2, &sub2));
}

#[test]
fn mismatched_types_should_fail() {
    let sup = parse("{a:1}");
    let sub = parse("[1]");
    assert!(!nbt_is_subset(&sup, &sub));
}

#[test]
fn empty_list_subset() {
    let sup = parse("[1,2,3]");
    let sub = parse("[]");
    assert!(nbt_is_subset(&sup, &sub));
}

#[test]
fn non_empty_list_on_empty_should_fail() {
    let sup = parse("[]");
    let sub = parse("[1]");
    assert!(!nbt_is_subset(&sup, &sub));
}

#[test]
fn empty_compound_subset() {
    let sup = parse("{a:1}");
    let sub = parse("{}");
    assert!(nbt_is_subset(&sup, &sub));
}

#[test]
fn byte_array_exact_match() {
    let sup = parse("[I;1,2,3]");
    let sub = parse("[I;1,2,3]");
    assert!(nbt_is_subset(&sup, &sub));
}

#[test]
fn byte_array_partial_should_fail() {
    let sup = parse("[I;1,2,3]");
    let sub = parse("[I;2,3]");
    assert!(!nbt_is_subset(&sup, &sub));
}

#[test]
fn byte_array_missing_element_should_fail() {
    let sup = parse("[I;1,2]");
    let sub = parse("[I;1,2,3]");
    assert!(!nbt_is_subset(&sup, &sub));
}

#[test]
fn mixed_list_types_should_fail_to_parse() {
    let res = valence_nbt::snbt::from_snbt_str("[1, \"a\"]");
    assert!(res.is_err(), "Mixed-type list unexpectedly parsed");
}

#[test]
fn int_array_vs_byte_array_should_fail() {
    let sup = parse("[I;1,2,3]");
    let sub = parse("[B;1b,2b,3b]");
    assert!(!nbt_is_subset(&sup, &sub));
}

#[test]
fn nested_empty_compound() {
    let sup = parse("{a:{b:{}}}");
    let sub = parse("{a:{b:{}}}");
    assert!(nbt_is_subset(&sup, &sub));
}

#[test]
fn deeply_nested_empty_list() {
    let sup = parse("{a:{b:[[],[1]]}}");
    let sub = parse("{a:{b:[[]]}}");
    assert!(nbt_is_subset(&sup, &sub));
}

#[test]
fn numeric_type_coercion_should_fail() {
    let res = valence_nbt::snbt::from_snbt_str("[1b, 2, 3s]");
    assert!(
        res.is_err(),
        "Parser unexpectedly accepted mixed numeric types"
    );
}

#[test]
fn long_array_partial_should_fail() {
    let sup = parse("[L;9223372036854775807l,0l]");
    let sub = parse("[L;0l]");
    assert!(!nbt_is_subset(&sup, &sub));
}

#[test]
fn empty_string_vs_non_empty_should_fail() {
    let sup = parse("{text:\"\"}");
    let sub = parse("{text:\"something\"}");
    assert!(!nbt_is_subset(&sup, &sub));
}

#[test]
fn float_vs_double_zero_should_fail() {
    let sup = parse("{val:0.0f}");
    let sub = parse("{val:0.0d}");
    assert!(!nbt_is_subset(&sup, &sub));
}

#[test]
fn compound_with_empty_list_and_nested_empty_compound() {
    let sup = parse("{data:{items:[], meta:{}}}");
    let sub = parse("{data:{items:[]}}");
    assert!(nbt_is_subset(&sup, &sub));
}

#[test]
fn unicode_string_match() {
    let sup = parse("{msg:\"こんにちは\"}");
    let sub = parse("{msg:\"こんにちは\"}");
    assert!(nbt_is_subset(&sup, &sub));
}
