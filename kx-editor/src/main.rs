use anyhow::Result;
use kx_engine::renderer::Graphics;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Window, WindowId},
};
struct App {
    graphics: Option<Graphics>,
    window: Option<Window>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            graphics: None,
            window: None,
        }
    }
}

impl App {
    fn resize(&mut self, _width: u32, _height: u32) {}

    fn draw(&mut self) {
        if let Some(graphics) = &mut self.graphics {
            graphics.draw().unwrap();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title("kx-editor")
            .with_inner_size(LogicalSize::new(1024, 768));

        let window = event_loop
            .create_window(window_attributes)
            .expect("failed to create window");

        let graphics = Graphics::new(
            window.window_handle().unwrap(),
            window.display_handle().unwrap(),
        )
        .expect("failed to create vulkan context");

        self.graphics = Some(graphics);
        self.window = Some(window);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
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
