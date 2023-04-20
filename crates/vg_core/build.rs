use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Emit the instructions
    vergen::EmitBuilder::builder().all_build().all_cargo().all_git().all_rustc().all_sysinfo().emit()?;
    Ok(())
}
