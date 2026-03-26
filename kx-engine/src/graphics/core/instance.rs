use anyhow::Result;
use std::ffi::{CStr, CString, c_void};

use ash::{ext, khr, vk};
use raw_window_handle::{DisplayHandle, WindowHandle};

unsafe extern "system" fn default_debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    ty: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut c_void,
) -> vk::Bool32 {
    let data = unsafe { &*data };
    let message = unsafe { CStr::from_ptr(data.p_message) }.to_string_lossy();
    eprintln!("[{severity:?}][{ty:?}] {message}");
    vk::FALSE
}

pub struct Instance {
    _entry: ash::Entry,
    instance: ash::Instance,
    surface: vk::SurfaceKHR,
    surface_fn: khr::surface::Instance,
    debug_utils_fn: Option<ext::debug_utils::Instance>,
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
}

impl Instance {
    pub fn surface(&self) -> vk::SurfaceKHR {
        self.surface
    }

    pub fn surface_fn(&self) -> &khr::surface::Instance {
        &self.surface_fn
    }

    pub fn destroy(&self) {
        unsafe {
            if let Some((loader, messenger)) =
                self.debug_utils_fn.as_ref().zip(self.debug_messenger)
            {
                loader.destroy_debug_utils_messenger(messenger, None);
            }
            self.surface_fn.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

impl std::ops::Deref for Instance {
    type Target = ash::Instance;
    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

pub struct InstanceBuilder {
    app_name: String,
    engine_name: String,
    api_version: u32,
    validation: bool,
    debug_messenger: bool,
}

impl InstanceBuilder {
    pub fn new() -> Self {
        Self {
            app_name: String::new(),
            engine_name: String::new(),
            api_version: vk::API_VERSION_1_3,
            validation: false,
            debug_messenger: false,
        }
    }

    pub fn app_name(mut self, name: &str) -> Self {
        self.app_name = name.into();
        self
    }

    pub fn engine_name(mut self, name: &str) -> Self {
        self.engine_name = name.into();
        self
    }

    pub fn api_version(mut self, version: u32) -> Self {
        self.api_version = version;
        self
    }

    pub fn validation(mut self, enable: bool) -> Self {
        self.validation = enable;
        self
    }

    pub fn debug_messenger(mut self, enable: bool) -> Self {
        self.debug_messenger = enable;
        self
    }

    pub fn build(self, window: WindowHandle, display: DisplayHandle) -> Result<Instance> {
        let entry = unsafe { ash::Entry::load()? };

        let app_name = CString::new(self.app_name)?;
        let engine_name = CString::new(self.engine_name)?;

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .engine_name(&engine_name)
            .api_version(self.api_version);

        let surface_extensions = ash_window::enumerate_required_extensions(display.as_raw())?;
        let mut extensions: Vec<*const _> = surface_extensions.to_vec();
        let mut layers: Vec<*const _> = Vec::new();

        if self.validation {
            layers.push(c"VK_LAYER_KHRONOS_validation".as_ptr());
        }
        if self.debug_messenger {
            extensions.push(vk::EXT_DEBUG_UTILS_NAME.as_ptr());
        }

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layers);

        let instance = unsafe { entry.create_instance(&create_info, None)? };

        let surface_fn = khr::surface::Instance::new(&entry, &instance);
        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, display.as_raw(), window.as_raw(), None)?
        };

        let mut debug_utils_fn = None;
        let mut debug_messenger_handle = None;

        if self.debug_messenger {
            let loader = ext::debug_utils::Instance::new(&entry, &instance);
            let info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(default_debug_callback));

            let messenger = unsafe { loader.create_debug_utils_messenger(&info, None)? };
            debug_utils_fn = Some(loader);
            debug_messenger_handle = Some(messenger);
        }

        Ok(Instance {
            _entry: entry,
            instance,
            surface,
            surface_fn,
            debug_utils_fn,
            debug_messenger: debug_messenger_handle,
        })
    }
}
