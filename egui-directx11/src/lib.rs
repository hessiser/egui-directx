macro_rules! expect {
    ($val:expr, $msg:expr) => {
            $val.expect($msg)
        // if cfg!(feature = "silent") {
        //     $val.unwrap()
        // } else {
        //     $val.expect($msg)
        // }
    };
}


mod backup;
mod mesh;
mod painter;
mod shader;
mod texture;
pub mod app;
pub mod input_manager;

pub use painter::*;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Unrecoverable error occured {0}")]
    General(&'static str),

    #[error("Windows error {0}")]
    Win(#[from] windows::core::Error),
}
