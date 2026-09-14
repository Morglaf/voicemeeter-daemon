use crate::config::Config;
use crate::mini_keyboard::key_capture::{
    begin_capture, cancel_capture, format_captured, poll_capture, try_egui_key, CapturedKeys,
};
use crate::mini_keyboard::mapping::{MappedAction, PhysicalKey};
use crate::scenes::{Scene, SceneTrigger, TitleMatcher};
use egui_wgpu::ScreenDescriptor;
use egui_winit::egui;
use std::sync::Arc;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionKind {
    None,
    KeyCombo,
    SimpleKey,
    MouseWheelUp,
    MouseWheelDown,
    ArrowLeft,
    ArrowRight,
    LaunchProgram,
    RunCommand,
    SwitchScene,
    CycleScene,
}

pub struct ConfigGui {
    context: egui::Context,
    egui_state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    visible: bool,
    pending_save: bool,
    editor: GuiEditor,
}

struct GuiEditor {
    selected_scene_id: String,
    selected_key: PhysicalKey,
    action_kind: ActionKind,
    capturing: bool,
    new_scene_name: String,
    edit_process: String,
    edit_title: String,
    edit_title_is_regex: bool,
    edit_priority: i32,
    edit_launch_path: String,
    edit_launch_args: String,
    edit_command: String,
    edit_use_shell: bool,
    edit_switch_scene_id: String,
    captured_keys: Option<CapturedKeys>,
}

impl ConfigGui {
    pub fn new(window: &Arc<Window>, config: &Config) -> Result<Self, String> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("surface wgpu: {e}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| "adaptateur wgpu introuvable".to_string())?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("vm-controller-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .map_err(|e| format!("device wgpu: {e}"))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let context = egui::Context::default();
        let egui_state = egui_winit::State::new(
            context.clone(),
            egui::viewport::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);

        let selected_scene_id = config.scenes.active_id.clone();
        let mut editor = GuiEditor::new(selected_scene_id);
        editor.load_key_editor(config);

        Ok(Self {
            context,
            egui_state,
            renderer,
            surface,
            surface_config,
            device,
            queue,
            visible: false,
            pending_save: false,
            editor,
        })
    }

    pub fn show(&mut self, window: &Window) {
        self.visible = true;
        window.set_visible(true);
        let _ = window.focus_window();
        window.request_redraw();
    }

    pub fn hide(&mut self, window: &Window) {
        self.visible = false;
        window.set_visible(false);
    }

    pub fn take_pending_save(&mut self) -> bool {
        let v = self.pending_save;
        self.pending_save = false;
        v
    }

    pub fn on_winit_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if !self.editor.capturing {
            return false;
        }
        if let Some(keys) = crate::mini_keyboard::key_capture::try_winit_key(event) {
            self.editor.captured_keys = Some(keys);
            self.editor.capturing = false;
            cancel_capture();
            return true;
        }
        false
    }

    pub fn handle_window_event(
        &mut self,
        window: &Window,
        event: &WindowEvent,
    ) -> egui_winit::EventResponse {
        self.egui_state.on_window_event(window, event)
    }

    pub fn on_resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    pub fn render(&mut self, window: &Window, config: &mut Config) {
        if !self.visible {
            return;
        }

        if let Some(keys) = poll_capture() {
            self.editor.captured_keys = Some(keys);
            self.editor.capturing = false;
        }

        let raw_input = self.egui_state.take_egui_input(window);
        let context = &self.context;
        let editor = &mut self.editor;
        let mut save = false;
        let full_output = context.run(raw_input, |ctx| {
            if editor.draw(ctx, config) {
                save = true;
            }
        });
        if save {
            self.pending_save = true;
        }
        self.egui_state
            .handle_platform_output(window, full_output.platform_output);

        let pixels_per_point = window.scale_factor() as f32;
        let width = self.surface_config.width;
        let height = self.surface_config.height;
        let screen_desc = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point,
        };

        let output = self
            .context
            .tessellate(full_output.shapes, pixels_per_point);

        for (id, delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        let surface_texture = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.surface_config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                eprintln!("GPU: mémoire insuffisante");
                return;
            }
            Err(_) => return,
        };

        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("egui"),
            });

        self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &output,
            &screen_desc,
        );

        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.1,
                                g: 0.1,
                                b: 0.12,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.renderer
                .render(&mut pass, &output, &screen_desc);
        }

        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }

        window.request_redraw();
    }
}

impl GuiEditor {
    fn new(selected_scene_id: String) -> Self {
        Self {
            selected_scene_id,
            selected_key: PhysicalKey::Prev,
            action_kind: ActionKind::None,
            capturing: false,
            new_scene_name: String::new(),
            edit_process: String::new(),
            edit_title: String::new(),
            edit_title_is_regex: false,
            edit_priority: 0,
            edit_launch_path: String::new(),
            edit_launch_args: String::new(),
            edit_command: String::new(),
            edit_use_shell: true,
            edit_switch_scene_id: String::new(),
            captured_keys: None,
        }
    }

    fn draw(&mut self, ctx: &egui::Context, config: &mut Config) -> bool {
        let mut pending_save = false;

        if self.capturing {
            self.try_capture_egui(ctx);
        }
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.heading("Voicemeeter Controller — Configuration");
            ui.label("Les combos utilisent des touches virtuelles (VK), pas la disposition AZERTY.");
            ui.checkbox(&mut config.scenes.auto_switch, "Bascule auto par application");
        });

        egui::SidePanel::left("scenes").show(ctx, |ui| {
            ui.heading("Scènes");
            ui.separator();
            for scene in &config.scenes.items {
                let selected = scene.id == self.selected_scene_id;
                if ui.selectable_label(selected, &scene.name).clicked() {
                    self.selected_scene_id = scene.id.clone();
                    self.load_key_editor(config);
                    pending_save = true;
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.new_scene_name);
                if ui.button("+ Nouvelle").clicked() && !self.new_scene_name.trim().is_empty() {
                    let scene = Scene::new(self.new_scene_name.trim());
                    self.selected_scene_id = scene.id.clone();
                    config.scenes.items.push(scene);
                    self.new_scene_name.clear();
                    pending_save = true;
                    self.load_key_editor(config);
                }
            });
            if ui.button("Supprimer la scène").clicked() {
                if config.scenes.items.len() > 1 {
                    config.scenes.items.retain(|s| s.id != self.selected_scene_id);
                    config.scenes.ensure_valid();
                    self.selected_scene_id = config.scenes.active_id.clone();
                    pending_save = true;
                    self.load_key_editor(config);
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(scene_idx) = config
                .scenes
                .items
                .iter()
                .position(|s| s.id == self.selected_scene_id)
            else {
                ui.label("Sélectionnez une scène.");
                return;
            };

            let scene_name = config.scenes.items[scene_idx].name.clone();
            ui.heading(format!("Mapping — {scene_name}"));
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Touche :");
                for key in PhysicalKey::ALL {
                    if ui
                        .selectable_label(self.selected_key == key, key.label())
                        .clicked()
                    {
                        self.selected_key = key;
                        self.load_key_editor(config);
                    }
                }
            });

            ui.separator();
            ui.label("Type d'action :");
            egui::ComboBox::from_id_salt("action_kind")
                .selected_text(action_kind_label(self.action_kind))
                .show_ui(ui, |ui| {
                    for kind in ACTION_KINDS {
                        ui.selectable_value(&mut self.action_kind, kind, action_kind_label(kind));
                    }
                });

            match self.action_kind {
                ActionKind::None => {}
                ActionKind::KeyCombo | ActionKind::SimpleKey => {
                    if ui.button("Capturer touche").clicked() {
                        begin_capture();
                        self.capturing = true;
                    }
                    if self.capturing {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "Appuyez sur une touche (focus sur cette fenêtre)…",
                        );
                    } else if let Some(keys) = &self.captured_keys {
                        ui.label(format!("Capturé : {}", format_captured(keys)));
                    }
                }
                ActionKind::LaunchProgram => {
                    ui.horizontal(|ui| {
                        ui.label("Chemin :");
                        ui.text_edit_singleline(&mut self.edit_launch_path);
                        if ui.button("Parcourir…").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                self.edit_launch_path = path.display().to_string();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Arguments :");
                        ui.text_edit_singleline(&mut self.edit_launch_args);
                    });
                }
                ActionKind::RunCommand => {
                    ui.label("Commande :");
                    ui.text_edit_multiline(&mut self.edit_command);
                    ui.checkbox(&mut self.edit_use_shell, "Utiliser cmd /C");
                }
                ActionKind::SwitchScene => {
                    egui::ComboBox::from_id_salt("switch_scene")
                        .selected_text(
                            config
                                .scenes
                                .items
                                .iter()
                                .find(|s| s.id == self.edit_switch_scene_id)
                                .map(|s| s.name.as_str())
                                .unwrap_or("Choisir…"),
                        )
                        .show_ui(ui, |ui| {
                            for scene in &config.scenes.items {
                                ui.selectable_value(
                                    &mut self.edit_switch_scene_id,
                                    scene.id.clone(),
                                    &scene.name,
                                );
                            }
                        });
                }
                ActionKind::CycleScene => {
                    ui.label("Passe à la scène suivante dans la liste.");
                }
                ActionKind::MouseWheelUp | ActionKind::MouseWheelDown => {
                    ui.label("Molette avec Ctrl maintenu.");
                }
                ActionKind::ArrowLeft | ActionKind::ArrowRight => {}
            }

            if ui.button("Appliquer au mapping").clicked() {
                let action = self.build_action();
                config.scenes.items[scene_idx].set_mapping(self.selected_key, action);
                pending_save = true;
            }

            let current = config.scenes.items[scene_idx].mapping_for(self.selected_key);
            ui.separator();
            ui.label(format!("Actuel : {}", current.label()));

            ui.add_space(12.0);
            ui.heading("Déclencheurs automatiques");
            ui.label("Laissez vide pour ne pas déclencher par application.");

            ui.horizontal(|ui| {
                ui.label("Processus :");
                ui.text_edit_singleline(&mut self.edit_process)
                    .on_hover_text("ex. photoshop.exe");
            });
            ui.horizontal(|ui| {
                ui.label("Titre fenêtre :");
                ui.text_edit_singleline(&mut self.edit_title);
                ui.checkbox(&mut self.edit_title_is_regex, "Regex");
            });
            ui.horizontal(|ui| {
                ui.label("Priorité :");
                ui.add(egui::DragValue::new(&mut self.edit_priority));
            });

            if ui.button("Enregistrer déclencheur").clicked() {
                let trigger = SceneTrigger {
                    process: if self.edit_process.trim().is_empty() {
                        None
                    } else {
                        Some(self.edit_process.trim().to_string())
                    },
                    window_title: if self.edit_title.trim().is_empty() {
                        None
                    } else if self.edit_title_is_regex {
                        Some(TitleMatcher::Regex(self.edit_title.trim().to_string()))
                    } else {
                        Some(TitleMatcher::Contains(self.edit_title.trim().to_string()))
                    },
                };
                if trigger.process.is_some() || trigger.window_title.is_some() {
                    config.scenes.items[scene_idx].priority = self.edit_priority;
                    config.scenes.items[scene_idx].triggers.push(trigger);
                    self.edit_process.clear();
                    self.edit_title.clear();
                    pending_save = true;
                }
            }

            let triggers: Vec<SceneTrigger> =
                config.scenes.items[scene_idx].triggers.clone();
            let mut remove_idx = None;
            for (i, trigger) in triggers.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(trigger_summary(trigger));
                    if ui.button("✕").clicked() {
                        remove_idx = Some(i);
                    }
                });
            }
            if let Some(i) = remove_idx {
                config.scenes.items[scene_idx].triggers.remove(i);
                pending_save = true;
            }
        });
        pending_save
    }

    fn try_capture_egui(&mut self, ctx: &egui::Context) {
        let mut found = None;
        ctx.input(|input| {
            for event in &input.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    physical_key,
                    ..
                } = event
                {
                    let lookup = physical_key.unwrap_or(*key);
                    if let Some(keys) = try_egui_key(lookup, *modifiers) {
                        found = Some(keys);
                        break;
                    }
                }
            }
        });
        if let Some(keys) = found {
            self.captured_keys = Some(keys);
            self.capturing = false;
            cancel_capture();
        }
    }

    fn load_key_editor(&mut self, config: &Config) {
        let action = config
            .scenes
            .scene_by_id(&self.selected_scene_id)
            .map(|s| s.mapping_for(self.selected_key))
            .unwrap_or(MappedAction::None);
        self.action_kind = action_to_kind(&action);
        self.captured_keys = None;
        match &action {
            MappedAction::KeyCombo {
                vk,
                ctrl,
                alt,
                shift,
            } => {
                self.captured_keys = Some(CapturedKeys {
                    vk: *vk,
                    ctrl: *ctrl,
                    alt: *alt,
                    shift: *shift,
                });
            }
            MappedAction::SimpleKey { vk } => {
                self.captured_keys = Some(CapturedKeys {
                    vk: *vk,
                    ctrl: false,
                    alt: false,
                    shift: false,
                });
            }
            MappedAction::LaunchProgram { path, args } => {
                self.edit_launch_path = path.clone();
                self.edit_launch_args = args.join(" ");
            }
            MappedAction::RunCommand { line, use_shell } => {
                self.edit_command = line.clone();
                self.edit_use_shell = *use_shell;
            }
            MappedAction::SwitchScene { scene_id } => {
                self.edit_switch_scene_id = scene_id.clone();
            }
            _ => {}
        }
        if let Some(scene) = config.scenes.scene_by_id(&self.selected_scene_id) {
            self.edit_priority = scene.priority;
        }
    }

    fn build_action(&self) -> MappedAction {
        match self.action_kind {
            ActionKind::None => MappedAction::None,
            ActionKind::KeyCombo => {
                let keys = self.captured_keys.clone().unwrap_or(CapturedKeys {
                    vk: 0x41,
                    ctrl: true,
                    alt: false,
                    shift: false,
                });
                MappedAction::KeyCombo {
                    vk: keys.vk,
                    ctrl: keys.ctrl,
                    alt: keys.alt,
                    shift: keys.shift,
                }
            }
            ActionKind::SimpleKey => {
                let vk = self.captured_keys.as_ref().map(|k| k.vk).unwrap_or(0x41);
                MappedAction::SimpleKey { vk }
            }
            ActionKind::MouseWheelUp => MappedAction::MouseWheel {
                delta: 120,
                ctrl: true,
            },
            ActionKind::MouseWheelDown => MappedAction::MouseWheel {
                delta: -120,
                ctrl: true,
            },
            ActionKind::ArrowLeft => MappedAction::ArrowLeft,
            ActionKind::ArrowRight => MappedAction::ArrowRight,
            ActionKind::LaunchProgram => MappedAction::LaunchProgram {
                path: self.edit_launch_path.clone(),
                args: self
                    .edit_launch_args
                    .split_whitespace()
                    .map(str::to_string)
                    .collect(),
            },
            ActionKind::RunCommand => MappedAction::RunCommand {
                line: self.edit_command.clone(),
                use_shell: self.edit_use_shell,
            },
            ActionKind::SwitchScene => MappedAction::SwitchScene {
                scene_id: self.edit_switch_scene_id.clone(),
            },
            ActionKind::CycleScene => MappedAction::CycleScene,
        }
    }
}

const ACTION_KINDS: [ActionKind; 11] = [
    ActionKind::None,
    ActionKind::KeyCombo,
    ActionKind::SimpleKey,
    ActionKind::MouseWheelUp,
    ActionKind::MouseWheelDown,
    ActionKind::ArrowLeft,
    ActionKind::ArrowRight,
    ActionKind::LaunchProgram,
    ActionKind::RunCommand,
    ActionKind::SwitchScene,
    ActionKind::CycleScene,
];

fn action_kind_label(kind: ActionKind) -> &'static str {
    match kind {
        ActionKind::None => "Aucune",
        ActionKind::KeyCombo => "Combinaison de touches",
        ActionKind::SimpleKey => "Touche simple",
        ActionKind::MouseWheelUp => "Molette haut (Ctrl)",
        ActionKind::MouseWheelDown => "Molette bas (Ctrl)",
        ActionKind::ArrowLeft => "Flèche gauche",
        ActionKind::ArrowRight => "Flèche droite",
        ActionKind::LaunchProgram => "Lancer un programme",
        ActionKind::RunCommand => "Exécuter une commande",
        ActionKind::SwitchScene => "Changer de scène",
        ActionKind::CycleScene => "Scène suivante",
    }
}

fn action_to_kind(action: &MappedAction) -> ActionKind {
    match action {
        MappedAction::None => ActionKind::None,
        MappedAction::KeyCombo { .. } => ActionKind::KeyCombo,
        MappedAction::SimpleKey { .. } => ActionKind::SimpleKey,
        MappedAction::MouseWheel { delta, .. } if *delta > 0 => ActionKind::MouseWheelUp,
        MappedAction::MouseWheel { .. } => ActionKind::MouseWheelDown,
        MappedAction::ArrowLeft => ActionKind::ArrowLeft,
        MappedAction::ArrowRight => ActionKind::ArrowRight,
        MappedAction::LaunchProgram { .. } => ActionKind::LaunchProgram,
        MappedAction::RunCommand { .. } => ActionKind::RunCommand,
        MappedAction::SwitchScene { .. } => ActionKind::SwitchScene,
        MappedAction::ToggleMode | MappedAction::CycleScene => ActionKind::CycleScene,
    }
}

fn trigger_summary(trigger: &SceneTrigger) -> String {
    let mut parts = Vec::new();
    if let Some(p) = &trigger.process {
        parts.push(format!("proc:{p}"));
    }
    if let Some(t) = &trigger.window_title {
        match t {
            TitleMatcher::Contains(s) => parts.push(format!("titre~{s}")),
            TitleMatcher::Regex(s) => parts.push(format!("regex:{s}")),
        }
    }
    parts.join(" + ")
}

pub fn create_config_window(event_loop: &ActiveEventLoop) -> Arc<Window> {
    let attrs = Window::default_attributes()
        .with_title("Voicemeeter Controller — Configuration")
        .with_inner_size(winit::dpi::LogicalSize::new(900.0, 600.0))
        .with_visible(false);
    Arc::new(
        event_loop
            .create_window(attrs)
            .expect("fenêtre configuration"),
    )
}
