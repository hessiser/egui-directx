// Frankensteined
// https://github.com/unknowntrojan/egui-d3d9 
// sy1ntexx's egui-d3d11
// https://github.com/ohchase/egui-directx

use std::ptr::null;

use egui::{epaint::Primitive, Context};
use windows::{core::{s, HRESULT}, Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM}, Graphics::Dxgi::IDXGISwapChain, UI::WindowsAndMessaging::GetClientRect},
};
use windows::Win32::Graphics::{Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST, Direct3D11::{ID3D11BlendState, ID3D11Device, ID3D11DeviceContext, ID3D11InputLayout, ID3D11RasterizerState, ID3D11RenderTargetView, ID3D11SamplerState, ID3D11Texture2D, D3D11_APPEND_ALIGNED_ELEMENT, D3D11_BLEND_DESC, D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_ONE, D3D11_BLEND_OP_ADD, D3D11_BLEND_SRC_ALPHA, D3D11_COLOR_WRITE_ENABLE_ALL, D3D11_COMPARISON_ALWAYS, D3D11_CULL_NONE, D3D11_FILL_SOLID, D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_INPUT_ELEMENT_DESC, D3D11_INPUT_PER_VERTEX_DATA, D3D11_RASTERIZER_DESC, D3D11_RENDER_TARGET_BLEND_DESC, D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_BORDER, D3D11_VIEWPORT}, Dxgi::Common::{DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_R32_UINT}};

use crate::{
    backup::BackupState, input_manager::{InputManager, InputResult}, mesh::{create_index_buffer, create_vertex_buffer, GpuMesh, GpuVertex}, shader::CompiledShaders, texture::TextureAllocator
};

const INPUT_ELEMENTS_DESC: [D3D11_INPUT_ELEMENT_DESC; 3] = [
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: s!("POSITION"),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: 0,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: s!("TEXCOORD"),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: D3D11_APPEND_ALIGNED_ELEMENT,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: s!("COLOR"),
        SemanticIndex: 0,
        Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: D3D11_APPEND_ALIGNED_ELEMENT,
        InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        InstanceDataStepRate: 0,
    },
];

pub struct EguiDx11<T> {
    render_view: Option<ID3D11RenderTargetView>,
    ui_fn: Box<dyn FnMut(&Context, &mut T) + 'static>,
    ui_state: T,
    pub hwnd: HWND,
    tex_alloc: TextureAllocator,
    input_layout: ID3D11InputLayout,
    input_manager: InputManager,
    shaders: CompiledShaders,
    backup: BackupState,
    ctx: Context,
    should_reset: bool,
    // // get it? tEx-man? tax-man? no?
    // tex_man: TextureManager,
    // buffers: Buffers,
    // prims: Vec<MeshDescriptor>,
    // last_idx_capacity: usize,
    // last_vtx_capacity: usize,
}

impl<T> EguiDx11<T> {
       /// Initializes application and state. You should call this only once!
       pub fn init_with_state_context(
        swap: &IDXGISwapChain,
        ui_fn: impl FnMut(&Context, &mut T) + 'static,
        ui_state: T,
        context: Context,
    ) -> Self {
        unsafe {

            let swap_desc = expect!(swap.GetDesc(), "Failed to get swapchain's descriptor");

            let hwnd = swap_desc.OutputWindow;
            if hwnd.0.is_null() {
                panic!("Invalid output window descriptor");
            }

            let dev: ID3D11Device = expect!(swap.GetDevice(), "Failed to get swapchain's device");

            let backbuffer: ID3D11Texture2D =
                expect!(swap.GetBuffer(0), "Failed to get swapchain's backbuffer");
                
            let mut render_view: Option<ID3D11RenderTargetView> = None;
            expect!(
                dev.CreateRenderTargetView(&backbuffer, None, Some(&mut render_view)),
                "Failed to create new render target view"
            );

            let shaders = CompiledShaders::new(&dev).unwrap();

            let mut input_layout: Option<ID3D11InputLayout> = None;
            expect!(
                dev.CreateInputLayout(&INPUT_ELEMENTS_DESC, shaders.bytecode(), Some(&mut input_layout)),
                "Failed to create input layout"
            );

            Self {
                ui_fn: Box::new(ui_fn),
                ui_state,
                hwnd,
                backup: BackupState::default(),
                tex_alloc: TextureAllocator::default(),
                input_manager: InputManager::new(hwnd),
                ctx: Context::default(),
                should_reset: false,
                input_layout: input_layout.unwrap(),
                render_view,
                shaders
            }
        }
    }

    /// Initializes application and state. Sets egui's context to default value. You should call this only once!
    pub fn init_with_state(
        swap: &IDXGISwapChain,
        ui: impl FnMut(&Context, &mut T) + 'static,
        state: T,
    ) -> Self {
        Self::init_with_state_context(swap, ui, state, Context::default())
    }

    pub fn present(&mut self, swap_chain: &IDXGISwapChain) {
        unsafe {
            let (dev, ctx) = &get_device_and_context(swap_chain);
            self.backup.save(ctx);

            let screen = self.get_screen_size();

            if cfg!(feature = "clear") {
                ctx.ClearRenderTargetView(self.render_view.as_ref().unwrap(), &[0.39, 0.58, 0.92, 1.0]);
            }

            let output = self.ctx.run(self.input_manager.collect_input(), |ctx| {
                // safe. present will never run in parallel.
                (self.ui_fn)(ctx, &mut self.ui_state)
            });

            if !output.textures_delta.is_empty() {
                self.tex_alloc
                    .process_deltas(dev, ctx, output.textures_delta).unwrap();
            }

            if self.should_reset {
                self.ctx.request_repaint();

                self.should_reset = false;
            }


            // // It this necessary?
            // if !output.platform_output.copied_text.is_empty() {
            //     let _ = WindowsClipboardContext.set_contents(output.platform_output.copied_text);
            // }

            let primitives = self
                .ctx
                .tessellate(output.shapes, output.pixels_per_point)
                .into_iter()
                .filter_map(|prim| {
                    if let Primitive::Mesh(mesh) = prim.primitive {
                        GpuMesh::from_mesh(screen, mesh, prim.clip_rect)
                    } else {
                        panic!("Paint callbacks are not yet supported")
                    }
                })
                .collect::<Vec<_>>();

            self.set_blend_state(dev, ctx);
            self.set_raster_options(dev, ctx);
            self.set_sampler_state(dev, ctx);

            ctx.RSSetViewports(Some(&[self.get_viewport()]));
            ctx.OMSetRenderTargets(Some(&[self.render_view.clone()]), None);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            ctx.IASetInputLayout(&self.input_layout);
            
            for mesh in primitives {
                let idx = create_index_buffer(dev, &mesh).unwrap();
                let vtx = create_vertex_buffer(dev, &mesh).unwrap();

                let texture = self.tex_alloc.get_by_id(mesh.texture_id);

                ctx.RSSetScissorRects(Some(&[RECT {
                    left: mesh.clip.left() as _,
                    top: mesh.clip.top() as _,
                    right: mesh.clip.right() as _,
                    bottom: mesh.clip.bottom() as _,
                }]));

                if texture.is_some() {
                    ctx.PSSetShaderResources(0, Some(&[texture]));
                }

                ctx.IASetVertexBuffers(0, 1, Some(&Some(vtx)), Some(&(size_of::<GpuVertex>() as _)), Some(&0));
                ctx.IASetIndexBuffer(&idx, DXGI_FORMAT_R32_UINT, 0);
                ctx.VSSetShader(&self.shaders.vertex, Some(&[]));
                ctx.PSSetShader(&self.shaders.pixel, None);

                ctx.DrawIndexed(mesh.indices.len() as _, 0, 0);
            }

            self.backup.restore(ctx);

        }

    }

    
    /// Call when resizing buffers.
    /// Do not call the original function before it, instead call it inside of the `original` closure.
    /// # Behavior
    /// In `origin` closure make sure to call the original `ResizeBuffers`.
    pub fn resize_buffers(
        &mut self,
        swap_chain: &IDXGISwapChain,
        original: impl FnOnce() -> HRESULT,
    ) -> HRESULT {
        unsafe {
            drop(self.render_view.take());

            let result = original();

            let backbuffer: ID3D11Texture2D = expect!(
                swap_chain.GetBuffer(0),
                "Failed to get swapchain's backbuffer"
            );

            let device: ID3D11Device =
                expect!(swap_chain.GetDevice(), "Failed to get swapchain's device");

            let mut new_view: Option<ID3D11RenderTargetView> = None;
            expect!(
                device.CreateRenderTargetView(&backbuffer, Some(null()), Some(&mut new_view)),
                "Failed to create render target view"
            );

            self.render_view = new_view;
            result
        }
    }

    #[inline]
    pub fn wnd_proc(&mut self, umsg: u32, wparam: WPARAM, lparam: LPARAM) -> InputResult {
        // safe. we only write here, and only read elsewhere.
        self.input_manager.process(umsg, wparam.0, lparam.0)
    }
}

unsafe fn get_device_and_context(swap: &IDXGISwapChain) -> (ID3D11Device, ID3D11DeviceContext) {
    let device: ID3D11Device = expect!(swap.GetDevice(), "Failed to get swapchain's device");
    let res =
    (
        expect!(device.GetImmediateContext(), "Failed to get device's immediate context"),
    );
    (device, res.0)
}


impl<T> EguiDx11<T> {
    fn get_screen_size(&self) -> (f32, f32) {
        let mut rect = RECT::default();
        unsafe {
            expect!(
                GetClientRect(self.hwnd, &mut rect),
                "Failed to GetClientRect()"
            );
        }
        (
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        )
    }

    fn get_viewport(&self) -> D3D11_VIEWPORT {
        let (w, h) = self.get_screen_size();
        D3D11_VIEWPORT {
            TopLeftX: 0.,
            TopLeftY: 0.,
            Width: w,
            Height: h,
            MinDepth: 0.,
            MaxDepth: 1.,
        }
    }

    fn set_blend_state(&self, dev: &ID3D11Device, ctx: &ID3D11DeviceContext) {
        let mut targets: [D3D11_RENDER_TARGET_BLEND_DESC; 8] = Default::default();
        targets[0].BlendEnable = true.into();
        targets[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
        targets[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
        targets[0].BlendOp = D3D11_BLEND_OP_ADD;
        targets[0].SrcBlendAlpha = D3D11_BLEND_ONE;
        targets[0].DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
        targets[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
        targets[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as _;

        let blend_desc = D3D11_BLEND_DESC {
            AlphaToCoverageEnable: false.into(),
            IndependentBlendEnable: false.into(),
            RenderTarget: targets,
        };

        unsafe {
            let mut blend_state: Option<ID3D11BlendState> = None;
            // Should I initialize as zero rray?
            let blend_factor = [0., 0., 0., 0.];
            expect!(
                dev.CreateBlendState(&blend_desc, Some(&mut blend_state)),
                "Failed to create blend state"
            );
            ctx.OMSetBlendState(Some(blend_state.as_ref().unwrap()), Some(&blend_factor), 0xffffffff);
        }
    }

    fn set_raster_options(&self, dev: &ID3D11Device, ctx: &ID3D11DeviceContext) {
        let raster_desc = D3D11_RASTERIZER_DESC {
            FillMode: D3D11_FILL_SOLID,
            CullMode: D3D11_CULL_NONE,
            FrontCounterClockwise: false.into(),
            DepthBias: false.into(),
            DepthBiasClamp: 0.,
            SlopeScaledDepthBias: 0.,
            DepthClipEnable: false.into(),
            ScissorEnable: true.into(),
            MultisampleEnable: false.into(),
            AntialiasedLineEnable: false.into(),
        };

        unsafe {
            let mut rasterizer_state: Option<ID3D11RasterizerState> = None;
            expect!(
                dev.CreateRasterizerState(&raster_desc, Some(&mut rasterizer_state)),
                "Failed to create rasterizer state"
            );
            ctx.RSSetState(rasterizer_state.as_ref().unwrap());
        }
    }

    fn set_sampler_state(&self, dev: &ID3D11Device, ctx: &ID3D11DeviceContext) {
        let desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_BORDER,
            AddressV: D3D11_TEXTURE_ADDRESS_BORDER,
            AddressW: D3D11_TEXTURE_ADDRESS_BORDER,
            MipLODBias: 0.,
            ComparisonFunc: D3D11_COMPARISON_ALWAYS,
            MinLOD: 0.,
            MaxLOD: 0.,
            BorderColor: [1., 1., 1., 1.],
            ..Default::default()
        };

        unsafe {
            let mut sampler_state: Option<ID3D11SamplerState> = None;
            expect!(dev.CreateSamplerState(&desc, Some(&mut sampler_state)), "Failed to create sampler");
            ctx.PSSetSamplers(0, Some(&[sampler_state]));
        }
    }
}