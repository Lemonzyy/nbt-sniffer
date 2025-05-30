use simdnbt::borrow::{NbtCompound, NbtList};
use valence_nbt::{Compound, List, Value};

pub fn convert_simdnbt_to_valence_nbt(compound: &NbtCompound) -> Value {
    let mut valence_compound = Compound::new();

    for (key, _) in compound.iter() {
        let key_str = key.to_string_lossy().into_owned();

        let valence_value = if let Some(b) = compound.byte(&key_str) {
            Value::Byte(b)
        } else if let Some(s) = compound.short(&key_str) {
            Value::Short(s)
        } else if let Some(i) = compound.int(&key_str) {
            Value::Int(i)
        } else if let Some(l) = compound.long(&key_str) {
            Value::Long(l)
        } else if let Some(f) = compound.float(&key_str) {
            Value::Float(f)
        } else if let Some(d) = compound.double(&key_str) {
            Value::Double(d)
        } else if let Some(arr) = compound.byte_array(&key_str) {
            let vec_i8 = arr.iter().map(|&b| b as i8).collect();
            Value::ByteArray(vec_i8)
        } else if let Some(s) = compound.string(&key_str) {
            Value::String(s.to_string_lossy().into_owned())
        } else if let Some(list) = compound.list(&key_str) {
            let valence_list = convert_list(&list);
            Value::List(valence_list)
        } else if let Some(c) = compound.compound(&key_str) {
            convert_simdnbt_to_valence_nbt(&c)
        } else if let Some(arr) = compound.int_array(&key_str) {
            Value::IntArray(arr.to_vec())
        } else if let Some(arr) = compound.long_array(&key_str) {
            Value::LongArray(arr.to_vec())
        } else {
            continue;
        };

        valence_compound.insert(key_str, valence_value);
    }

    Value::Compound(valence_compound)
}

pub fn convert_list(list: &NbtList) -> List {
    let mut valence_list = List::new();

    if let Some(bytes) = list.bytes() {
        for &b in bytes {
            let _ = valence_list.try_push(Value::Byte(b));
        }
    } else if let Some(shorts) = list.shorts() {
        for s in shorts {
            let _ = valence_list.try_push(Value::Short(s));
        }
    } else if let Some(ints) = list.ints() {
        for i in ints {
            let _ = valence_list.try_push(Value::Int(i));
        }
    } else if let Some(longs) = list.longs() {
        for l in longs {
            let _ = valence_list.try_push(Value::Long(l));
        }
    } else if let Some(floats) = list.floats() {
        for f in floats {
            let _ = valence_list.try_push(Value::Float(f));
        }
    } else if let Some(doubles) = list.doubles() {
        for d in doubles {
            let _ = valence_list.try_push(Value::Double(d));
        }
    } else if let Some(byte_arrays) = list.byte_arrays() {
        for arr in byte_arrays {
            let vec_i8 = arr.iter().map(|&b| b as i8).collect();
            let _ = valence_list.try_push(Value::ByteArray(vec_i8));
        }
    } else if let Some(strings) = list.strings() {
        for s in strings {
            let _ = valence_list.try_push(Value::String(s.to_string_lossy().into_owned()));
        }
    } else if let Some(lists) = list.lists() {
        for l in lists {
            let _ = valence_list.try_push(Value::List(convert_list(&l)));
        }
    } else if let Some(compounds) = list.compounds() {
        for c in compounds {
            let _ = valence_list.try_push(convert_simdnbt_to_valence_nbt(&c));
        }
    } else if let Some(int_arrays) = list.int_arrays() {
        for arr in int_arrays {
            let _ = valence_list.try_push(Value::IntArray(arr.to_vec()));
        }
    } else if let Some(long_arrays) = list.long_arrays() {
        for arr in long_arrays {
            let _ = valence_list.try_push(Value::LongArray(arr.to_vec()));
        }
    }

    valence_list
}
