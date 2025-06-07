use simdnbt::borrow::{NbtCompound, NbtList};
use valence_nbt::{Compound, List, Value};

pub const NBT_KEY_ID: &str = "id";
pub const NBT_KEY_COUNT: &str = "count";
pub const NBT_KEY_POS: &str = "Pos";
pub const NBT_KEY_ITEMS: &str = "Items";
pub const NBT_KEY_INVENTORY: &str = "Inventory";
pub const NBT_KEY_ITEM: &str = "Item";
pub const NBT_KEY_EQUIPMENT: &str = "equipment";
pub const NBT_KEY_PASSENGERS: &str = "Passengers";
pub const NBT_KEY_COMPONENTS: &str = "components";
pub const NBT_KEY_MINECRAFT_CONTAINER: &str = "minecraft:container";
pub const NBT_KEY_MINECRAFT_BUNDLE_CONTENTS: &str = "minecraft:bundle_contents";
pub const NBT_KEY_ENDER_ITEMS: &str = "EnderItems";
pub const NBT_KEY_PLAYER_DATA: &str = "Data"; // For level.dat
pub const NBT_KEY_PLAYER: &str = "Player"; // For level.dat, nested under "Data"

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

/// Extracts a UUID string from an NBT compound.
/// It checks for an Int Array named `UUID` (e.g. `[I;-132296786,2112623056,-1486552928,-920753162]`), which is the standard for modern Minecraft versions.
pub fn get_uuid_from_nbt(nbt_compound: &NbtCompound) -> Option<String> {
    // Standard Int Array "UUID" (common in entity NBT and player NBT in modern versions)
    if let Some(uuid_int_array) = nbt_compound.int_array("UUID")
        && uuid_int_array.len() == 4
    {
        let most_significant_long =
            ((uuid_int_array[0] as u64) << 32) | (uuid_int_array[1] as u64 & 0xFFFFFFFF);
        let least_significant_long =
            ((uuid_int_array[2] as u64) << 32) | (uuid_int_array[3] as u64 & 0xFFFFFFFF);
        let uuid_val = uuid::Uuid::from_u64_pair(most_significant_long, least_significant_long);
        return Some(uuid_val.to_string());
    }

    None
}

/// Helper to get a formatted string for an entity's position.
pub fn get_entity_pos_string(entity_nbt: &simdnbt::borrow::NbtCompound) -> Option<String> {
    entity_nbt
        .list(NBT_KEY_POS)
        .and_then(|pos_list| pos_list.doubles())
        .filter(|doubles| doubles.len() >= 3)
        .map(|doubles| format!("{:.2} {:.2} {:.2}", doubles[0], doubles[1], doubles[2]))
}
