use uesave::{games::palworld::Palworld, SaveGameArchiveType};
pub type Arch=SaveGameArchiveType<Palworld>;
pub type Properties=uesave::Properties<Arch>;
pub type Property=uesave::Property<Arch>;
pub type StructValue=uesave::StructValue<Arch>;
pub type ValueVec=uesave::ValueVec<Arch>;
pub type MapEntry=uesave::MapEntry<Arch>;

pub fn bag(p:&Property)->Option<&Properties>{if let Property::Struct(StructValue::Struct(v))=p {Some(v)}else{None}}
pub fn get<'a>(p:&'a Properties,path:&[&str])->Option<&'a Property>{
    let (first,rest)=path.split_first()?;
    let item=p.0.iter().find_map(|(key,value)|(key.1==*first).then_some(value))?;
    if rest.is_empty(){Some(item)}else{get(bag(item)?,rest)}
}
pub fn text(p:&Property)->Option<&str>{match p {Property::Str(v)|Property::Name(v)|Property::Enum(v)=>Some(v),_=>None}}
pub fn guid(p:&Property)->Option<String>{if let Property::Struct(StructValue::Guid(v))=p {(!v.is_nil()).then(||canonical(&v.to_string()))}else{None}}
pub fn canonical(s:&str)->String{s.to_ascii_lowercase().replace('-',"")}
pub fn valid_id(s:&str)->bool {let s=canonical(s);s.len()==32&&s.bytes().all(|c|c.is_ascii_hexdigit())}
pub fn maps(p:&Property)->Option<&Vec<MapEntry>>{if let Property::Map(v)=p{Some(v)}else{None}}
pub fn structs(p:&Property)->Option<&Vec<StructValue>>{if let Property::Array(ValueVec::Struct(v))=p{Some(v)}else{None}}
pub fn strings(p:&Property)->Option<&Vec<String>>{match p{Property::Array(ValueVec::Name(v)|ValueVec::Enum(v)|ValueVec::Str(v))=>Some(v),_=>None}}
pub fn boolean(p:&Property)->Option<bool>{if let Property::Bool(v)=p{Some(*v)}else{None}}
pub fn integer(p:&Property)->Option<i32>{if let Property::Int(v)=p{Some(*v)}else{None}}
