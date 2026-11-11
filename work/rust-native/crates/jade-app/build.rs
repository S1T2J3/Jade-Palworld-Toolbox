fn main(){
 println!("cargo:rerun-if-changed=assets/atlas.ico");
 if std::env::var("CARGO_CFG_TARGET_OS").as_deref()!=Ok("windows"){return;}
 let out=std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
 let icon=std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("assets/atlas.ico");
 let rc=out.join("icon.rc");let obj=out.join("icon.o");
 std::fs::write(&rc,format!("1 ICON \"{}\"\n",icon.to_string_lossy().replace('\\',"/"))).unwrap();
 let status=std::process::Command::new("llvm-windres").args(["-i"]).arg(&rc).arg("-o").arg(&obj).arg("-O").arg("coff").status().expect("install LLVM-MinGW windres to build the native Windows icon");
 assert!(status.success(),"Windows icon compilation failed");println!("cargo:rustc-link-arg={}",obj.display());
}
