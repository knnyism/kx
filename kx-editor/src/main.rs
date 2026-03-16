use anyhow::Result;
use kx_engine::renderer::Graphics;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Window, WindowId},
};

struct State {
    window: Arc<Window>,
    graphics: Graphics,
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl App {
    fn resize(&mut self, _width: u32, _height: u32) {}

    fn draw(&mut self) {}
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("kx-editor")
                .with_inner_size(LogicalSize::new(1280, 720));

            let window = Arc::new(
                event_loop
                    .create_window(window_attributes)
                    .expect("failed to create window"),
            );

            let graphics = Graphics::new(
                window.window_handle().unwrap(),
                window.display_handle().unwrap(),
            )
            .expect("failed to create vulkan context");

            self.state = Some(State { window, graphics });
        }
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => self.resize(size.width, size.height),
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}
