use anyhow::Result;
use cgmath::{vec2, vec3};
use imgui::{Context, DrawVert, Ui, sys::ImDrawVert};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use winit::window::Window;

use crate::engine::vertex::Vertex;

#[derive(Debug)]
pub struct GuiApp {
    pub context: Context,
    platform: WinitPlatform,
}

impl GuiApp {
    pub fn new(window: &Window) -> Self {
        let mut context = Context::create();

        let mut platform = WinitPlatform::init(&mut context);
        platform.attach_window(context.io_mut(), window, HiDpiMode::Default);

        Self { context, platform }
    }

    pub fn prepare(&mut self, window: &Window) -> Result<()> {
        self.platform
            .prepare_frame(self.context.io_mut(), &window)?;
        Ok(())
    }

    pub fn render(&mut self, window: &Window) -> Result<Vec<(Vec<Vertex>, Vec<u16>)>> {
        let ui = self.context.frame();
        self.platform.prepare_render(ui, window);
        let draw_data = self.context.render();

        let lists = draw_data
            .draw_lists()
            .map(|x| {
                (
                    x.vtx_buffer()
                        .to_vec()
                        .iter()
                        .map(|x| x.to_vertex())
                        .collect::<Vec<Vertex>>(),
                    x.idx_buffer().to_vec(),
                )
            })
            .collect::<Vec<(Vec<Vertex>, Vec<u16>)>>();

        Ok(lists)
    }
}

pub trait IntoVertex {
    fn to_vertex(&self) -> Vertex;
}

impl IntoVertex for DrawVert {
    fn to_vertex(&self) -> Vertex {
        Vertex::new(
            vec3(self.pos[0], self.pos[1], 0.0),
            vec3(
                self.col[0] as f32 / 255.0,
                self.col[1] as f32 / 255.0,
                self.col[2] as f32 / 255.0,
            ),
            vec2(self.uv[0], self.uv[1]),
        )
    }
}
