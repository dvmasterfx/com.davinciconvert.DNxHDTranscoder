use anyhow::{Context, Result};
use gtk4::{gio, glib};
use gtk4::prelude::*;
use gtk4::prelude::{ApplicationExt, ApplicationExtManual, DialogExt, FileExt, ListBoxRowExt, ListModelExt, Cast};
use gtk4::{Application, ApplicationWindow, Button, FileChooserAction, FileChooserDialog, FileFilter, Orientation, Box as GtkBox, Label, ListBox, ListBoxRow, ProgressBar};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::cell::RefCell;
use percent_encoding::percent_decode_str;


#[derive(Clone, Default, Debug)]
struct AppState {
    files: Vec<PathBuf>,
    output_dir: Option<PathBuf>,
    profile: String, // dnxhr_* perfil
    container: String, // mov | mxf
    audio_bits: u32,   // 16 | 24
    audio_channels: u32, // 2 | 4 | 8
    preserve_fps: bool,
    target_fps: f64,
    set_timecode: bool,
    timecode: String, // HH:MM:SS:FF
    normalize_ebu_r128: bool,
}

fn main() -> Result<()> {
    let app = Application::new(Some("com.davinciconvert.DNxHDTranscoder"), Default::default());

    app.connect_activate(|app| {
        if let Err(e) = build_ui(app) {
            eprintln!("Error starting UI: {e}");
        }
    });

    app.run();
    Ok(())
}

fn build_ui(app: &Application) -> Result<()> {

    let state = Arc::new(Mutex::new(AppState::default()));

    let window = ApplicationWindow::builder()
    .application(app)
    .title("DNxHD Transcoder")
    .default_width(800)
    .default_height(700)
    .build();

    // Add CSS provider for dark/light theme support
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        "
        progressbar {
            min-height: 30px;
            border-radius: 8px;
            border: 1px solid #ccc;
        }
        progressbar > trough {
            min-height: 20px;
            background-color: @theme_bg_color;
            border-radius: 8px;
        }
        progressbar > trough > progress {
            min-height: 10px;
            border-radius: 8px;
            background-color: @theme_selected_bg_color;
        }
        ",
        );

    // Apply the provider to the display (using non-deprecated function)
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
    }

     // Detect and follow system color scheme
     let settings = gtk4::Settings::default().unwrap();
     let prefers_dark = settings.is_gtk_application_prefer_dark_theme();
     settings.set_gtk_application_prefer_dark_theme(prefers_dark);

     let root = GtkBox::new(Orientation::Vertical, 8);
     root.set_margin_top(12);
     root.set_margin_bottom(12);
     root.set_margin_start(12);
     root.set_margin_end(12);

    // Controls
    let controls = GtkBox::new(Orientation::Horizontal, 8);
    let btn_select_files = Button::with_label("Select files...");
    let btn_select_output = Button::with_label("Output folder...");
    let btn_start = Button::with_label("Start");
    let btn_theme = Button::with_label("Toggle Theme");



    // Combo Profile DNxHR
    let combo_profile = gtk4::ComboBoxText::new();

    // Output container
    let combo_container = gtk4::ComboBoxText::new();
    for c in ["mov", "mxf"] { combo_container.append(Some(c), c); }
    combo_container.set_active_id(Some("mov"));

    // Audio Depth
    let combo_audio = gtk4::ComboBoxText::new();
    combo_audio.append(Some("16"), "PCM 16-bit");
    combo_audio.append(Some("24"), "PCM 24-bit");
    combo_audio.set_active_id(Some("16"));

    // Audio Channels
    let combo_channels = gtk4::ComboBoxText::new();
    for ch in [2,4,8] { combo_channels.append(Some(&ch.to_string()), &format!("{} ch", ch)); }
    combo_channels.set_active_id(Some("2"));

    // Timecode
    let chk_timecode = gtk4::CheckButton::with_label("Define timecode");
    let entry_timecode = gtk4::Entry::new();
    entry_timecode.set_text("00:00:00:00");
    entry_timecode.set_width_chars(10);
    entry_timecode.set_sensitive(false);

    // Normalization EBU R128
    let chk_normalize = gtk4::CheckButton::with_label("Normalize audio (EBU R128 -23 LUFS)");

    // Preserving FPS + FPS target
    let chk_preserve_fps = gtk4::CheckButton::with_label("Preserve FPS");
    chk_preserve_fps.set_active(true);
    let spin_fps = gtk4::SpinButton::with_range(1.0, 120.0, 0.1);
    spin_fps.set_value(25.0);
    spin_fps.set_sensitive(false);
    for (label, id) in [
    ("DNxHR LB (Low Bandwidth)", "dnxhr_lb"),
    ("DNxHR SQ (Standard Quality)", "dnxhr_sq"), 
    ("DNxHR HQ (High Quality)", "dnxhr_hq"),
    ("DNxHR HQX (High Quality 10-bit)", "dnxhr_hqx"),
    ("DNxHR 444 (4:4:4 10-bit)", "dnxhr_444")
    ] {
        combo_profile.append(Some(id), label);
    }

    combo_profile.set_active_id(Some("dnxhr_hq"));

    controls.append(&btn_select_files);
    controls.append(&btn_select_output);
    controls.append(&combo_profile);
    controls.append(&combo_container);
    controls.append(&combo_audio);
    controls.append(&combo_channels);
    controls.append(&chk_preserve_fps);
    controls.append(&spin_fps);
    controls.append(&chk_timecode);
    controls.append(&entry_timecode);
    controls.append(&chk_normalize);
    controls.append(&btn_start);

    // Output information
    let lbl_output = Label::new(Some("Output: (not selected)"));
    lbl_output.set_xalign(0.0);

    // Jobs List
    let list = ListBox::new();

    root.append(&controls);
    root.append(&lbl_output);
    root.append(&list);

    let vbox = GtkBox::new(Orientation::Vertical, 1);

    vbox.set_halign(gtk4::Align::Center);
    
    vbox.set_valign(gtk4::Align::Center);

    let label = Label::new(Some("Select or Drag file here!"));
    label.set_halign(gtk4::Align::Center);
    label.set_valign(gtk4::Align::Center);

    vbox.append(&label);
    root.append(&vbox);

// Initial state
{
    let mut st = state.lock().unwrap();
    st.profile = "dnxhr_hq".to_string();
    st.container = "mov".to_string();
    st.audio_bits = 16;
    st.audio_channels = 2;
    st.preserve_fps = true;
    st.target_fps = 25.0;
    st.set_timecode = false;
    st.timecode = "00:00:00:00".to_string();
    st.normalize_ebu_r128 = false;
}

    // Handler: change system Dark/Light Style
    {
        btn_theme.connect_clicked(move |_| {
            let current = settings.is_gtk_application_prefer_dark_theme();
            settings.set_gtk_application_prefer_dark_theme(!current);
        });
    }
    // Handler: change profile
    {
        let state = Arc::clone(&state);
        combo_profile.connect_changed(move |c| {
            if let Some(id) = c.active_id() {
                let mut st = state.lock().unwrap();
                st.profile = id.to_string();
            }
        });
    }

    // Handlers: container / audio / fps
    {
        let state = Arc::clone(&state);
        combo_container.connect_changed(move |c| {
            if let Some(id) = c.active_id() {
                let mut st = state.lock().unwrap();
                st.container = id.to_string();
            }
        });
    }
    {
        let state = Arc::clone(&state);
        combo_audio.connect_changed(move |c| {
            if let Some(id) = c.active_id() {
                let mut st = state.lock().unwrap();
                st.audio_bits = id.to_string().parse::<u32>().unwrap_or(16);
            }
        });
    }
    {
        let state = Arc::clone(&state);
        combo_channels.connect_changed(move |c| {
            if let Some(id) = c.active_id() {
                let mut st = state.lock().unwrap();
                st.audio_channels = id.to_string().parse::<u32>().unwrap_or(2);
            }
        });
    }
    {
        let state = Arc::clone(&state);
        let spin = spin_fps.clone();
        chk_preserve_fps.connect_toggled(move |chk| {
            let active = chk.is_active();
            spin.set_sensitive(!active);
            let mut st = state.lock().unwrap();
            st.preserve_fps = active;
        });
    }
    {
        let state = Arc::clone(&state);
        let entry_timecode_clone = entry_timecode.clone();
        chk_timecode.connect_toggled(move |chk| {
            let active = chk.is_active();
            entry_timecode_clone.set_sensitive(active);
            let mut st = state.lock().unwrap();
            st.set_timecode = active;
        });
    }
    {
        let state = Arc::clone(&state);
        entry_timecode.connect_changed(move |e| {
            let mut st = state.lock().unwrap();
            st.timecode = e.text().to_string();
        });
    }
    {
        let state = Arc::clone(&state);
        chk_normalize.connect_toggled(move |chk| {
            let mut st = state.lock().unwrap();
            st.normalize_ebu_r128 = chk.is_active();
        });
    }
    {
        let state = Arc::clone(&state);
        spin_fps.connect_value_changed(move |s| {
            let mut st = state.lock().unwrap();
            st.target_fps = s.value();
        });
    }

    // Handler: select files
    {
        let window = window.clone();
        let list = list.clone();
        let state = Arc::clone(&state);
        btn_select_files.connect_clicked(move |_| {
            let dlg = FileChooserDialog::new(
                Some("Select videos"),
                Some(&window),
                FileChooserAction::Open,
                &[("Cancel", gtk4::ResponseType::Cancel), ("Select", gtk4::ResponseType::Accept)],
                );
            dlg.set_modal(true);
            dlg.set_select_multiple(true);

            let filter_video = FileFilter::new();
            filter_video.add_pattern("*.mp4");
            filter_video.add_pattern("*.MP4");
            filter_video.add_pattern("*.mov");
            filter_video.add_pattern("*.MOV");
            filter_video.set_name(Some("Videos (*.mp4, *.mov)"));
            dlg.add_filter(&filter_video);

            dlg.connect_response({
                let list = list.clone();
                let state = Arc::clone(&state);
                move |dlg, resp| {
                    if resp == gtk4::ResponseType::Accept {
                        let model = dlg.files(); // gio::ListModel
                        let mut paths: Vec<PathBuf> = vec![];
                        let n = model.n_items();
                        for i in 0..n {
                            if let Some(obj) = model.item(i) {
                                if let Ok(file) = obj.downcast::<gio::File>() {
                                    if let Some(path) = file.path() {
                                        paths.push(path);
                                    }
                                }
                            }
                        }
                        if !paths.is_empty() {
                            {
                                let mut st = state.lock().unwrap();
                                st.files = paths.clone();
                            }
                            // clean list
                            clear_children(&list);
                            // Fill in
                            for p in paths {
                                let row = ListBoxRow::new();
                                let hb = GtkBox::new(Orientation::Horizontal, 8);
                                let label = Label::new(Some(p.file_name().and_then(|s| s.to_str()).unwrap_or("(no name)")));
                                label.set_xalign(0.0);
                                let progress = ProgressBar::new();
                                progress.add_css_class("progress");
                                progress.set_valign(gtk4::Align::Center);
                                progress.set_hexpand(true);
                                progress.set_show_text(true);
                                progress.set_text(Some("Waiting"));
                                hb.append(&label);
                                hb.append(&progress);
                                row.set_child(Some(&hb));
                                list.append(&row);
                            }
                        }
                    }
                    dlg.close();
                }
            });
            dlg.show();
        });
    }

    // Handler: select output folder
    {
        let window = window.clone();
        let state = Arc::clone(&state);
        let lbl_output = lbl_output.clone();
        btn_select_output.connect_clicked(move |_| {
            let dlg = FileChooserDialog::new(
                Some("Select output folder"),
                Some(&window),
                FileChooserAction::SelectFolder,
                &[("Cancel", gtk4::ResponseType::Cancel), ("Select", gtk4::ResponseType::Accept)],
                );
            dlg.set_modal(true);
            dlg.connect_response({
                let state = Arc::clone(&state);
                let lbl_output = lbl_output.clone();
                move |dlg, resp| {
                    if resp == gtk4::ResponseType::Accept {
                        if let Some(dir) = dlg.file().and_then(|f| f.path()) {
                            let mut st = state.lock().unwrap();
                            st.output_dir = Some(dir.clone());
                            lbl_output.set_text(&format!("Output: {}", dir.display()));
                        }
                    }
                    dlg.close();
                }
            });
            dlg.show();
        });
    }

    let header = gtk4::HeaderBar::new();
    header.pack_end(&btn_theme);
    window.set_titlebar(Some(&header));

    // Drag-and-drop (DnD) support for URIs
    {
        use gtk4::gdk::DragAction;
        let drop = gtk4::DropTarget::new(glib::types::Type::STRING, DragAction::COPY);
        let list_clone = list.clone();
        let state = Arc::clone(&state);
        drop.connect_drop(move |_, value, _, _| {
            if let Ok(text) = value.get::<String>() {
                // Parse text/uri-list (lines with file:///...)
                let mut added: Vec<PathBuf> = Vec::new();
                for line in text.lines() {
                    let s = line.trim();
                    if s.is_empty() || s.starts_with('#') { continue; }
                    // Remove prefix file://
                    let path = if let Some(stripped) = s.strip_prefix("file://") { stripped } else { s };
                    // Decode percent-encoding
                    let path = percent_decode_str(path)
                    .decode_utf8_lossy()
                    .to_string();

                    let pb = PathBuf::from(path);
                    if pb.is_file() {
                        added.push(pb);
                    }
                }
                if !added.is_empty() {
                    // Updates state and UI (adds to existing ones)
                    {
                        let mut st = state.lock().unwrap();
                        st.files.extend(added.clone());
                    }
                    for p in added {
                        let row = ListBoxRow::new();
                        let hb = GtkBox::new(Orientation::Horizontal, 8);
                        let label = Label::new(Some(p.file_name().and_then(|s| s.to_str()).unwrap_or("(no name)")));
                        label.set_xalign(0.0);
                        let progress = ProgressBar::new();
                        progress.set_valign(gtk4::Align::Center);
                        progress.set_hexpand(true);
                        progress.set_show_text(true);
                        progress.set_text(Some("Waiting"));
                        hb.append(&label);
                        hb.append(&progress);
                        row.set_child(Some(&hb));
                        list_clone.append(&row);
                    }
                }
            }
            true
        });
        window.add_controller(drop);
    }
    // Handler: start conversion
    {
        let list = list.clone();
        let state = Arc::clone(&state);
        btn_start.connect_clicked(move |_| {
            // Collect all progress bars by index
            let mut progress_bars = Vec::new();
            
            // Iterate through list items to find progress bars
            let mut child = list.first_child();
            while let Some(row_widget) = child {
                if let Ok(row) = row_widget.clone().downcast::<ListBoxRow>() {
                    if let Some(hbox) = row.child().and_then(|c| c.downcast::<GtkBox>().ok()) {
                        // Find the progress bar in this row
                        let mut hbox_child = hbox.first_child();
                        while let Some(widget) = hbox_child {
                            if let Ok(pb) = widget.clone().downcast::<ProgressBar>() {
                                progress_bars.push(pb);
                                break; // Found progress bar, move to next row
                            }
                            hbox_child = widget.next_sibling();
                        }
                    }
                }
                child = row_widget.next_sibling();
            }

            eprintln!("[DEBUG] Collected {} progress bars", progress_bars.len());

            let (tx, rx) = glib::MainContext::channel::<(usize, String, f64)>(0.into());
            
            // Background processing thread
            let state_bg = Arc::clone(&state);
            let tx_thread = tx.clone();
            std::thread::spawn(move || {
                let st = state_bg.lock().unwrap().clone();
                if st.files.is_empty() {
                    eprintln!("[DEBUG] No files to process");
                    return;
                }

                // Setup output directory
                let base_out = st.output_dir.clone().unwrap_or_else(|| {
                    st.files[0].parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
                });
                let out_dir = base_out.join("transcoded");
                let _ = std::fs::create_dir_all(&out_dir);

                for (idx, input) in st.files.iter().enumerate() {
                    let output_name = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
                    let ext = if st.container == "mxf" { "mxf" } else { "mov" };
                    let output = out_dir.join(format!("{}.{}", output_name, ext));

                    // Initial progress update
                    eprintln!("[DEBUG] Starting job {} for {}", idx, input.display());
                    let _ = tx_thread.send((idx, String::from("Starting..."), 0.01));

                    // Process file with progress updates
                    let tx_progress = tx_thread.clone();
                    match run_ffmpeg_with_progress(
                        input,
                        &output,
                        &st.profile,
                        st.audio_bits,
                        st.audio_channels,
                        st.preserve_fps,
                        st.target_fps,
                        st.set_timecode,
                        &st.timecode,
                        st.normalize_ebu_r128,
                        move |frac| {
                            let _ = tx_progress.send((idx, String::from("Converting..."), frac));
                        },
                        ) {
                        Ok(_) => {
                            eprintln!("[DEBUG] Job {} completed", idx);
                            let _ = tx_thread.send((idx, String::from("Completed"), 1.0));
                        }
                        Err(e) => {
                            eprintln!("[DEBUG] Job {} error: {}", idx, e);
                            let _ = tx_thread.send((idx, format!("Error: {}", e), 0.0));
                        }
                    }
                }
            });

            // UI update handler with indeterminate pulsing support using GLib channel
            // One timer per progress bar to pulse when duration is unknown
            let timers: Rc<RefCell<Vec<Option<glib::SourceId>>>> = Rc::new(RefCell::new((0..progress_bars.len()).map(|_| None).collect()));
            let timers_ui = Rc::clone(&timers);
            rx.attach(None, move |(idx, status, frac)| {
                eprintln!("[DEBUG] UI recv idx={} status='{}' frac={}", idx, status, frac);
                if let Some(pb) = progress_bars.get(idx) {
                    let pb = pb.clone();
                    if frac < 0.0 {
                        pb.set_text(Some(&status));
                        pb.set_show_text(true);
                        pb.set_pulse_step(0.02);
                        let mut timers_mut = timers_ui.borrow_mut();
                        if timers_mut.get(idx).and_then(|t| t.as_ref()).is_none() {
                            let pb_clone = pb.clone();
                            let source = glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                                pb_clone.pulse();
                                glib::ControlFlow::Continue
                            });
                            if let Some(slot) = timers_mut.get_mut(idx) {
                                *slot = Some(source);
                            }
                        }
                    } else {
                        let mut timers_mut = timers_ui.borrow_mut();
                        if let Some(slot) = timers_mut.get_mut(idx) {
                            if let Some(source) = slot.take() {
                                source.remove();
                            }
                        }
                        pb.set_fraction(frac.min(1.0));
                        pb.set_text(Some(&status));
                        pb.set_show_text(true);
                        pb.queue_draw();
                    }
                }
                glib::ControlFlow::Continue
            });
        });
}


window.set_child(Some(&root));
window.show();

Ok(())
}

fn clear_children(list: &ListBox) {
    let mut child = list.first_child();
    while let Some(c) = child {
        list.remove(&c);
        child = list.first_child();
    }
}

fn run_ffmpeg_with_progress(input: &Path, output: &Path, profile: &str, audio_bits: u32, audio_channels: u32, preserve_fps: bool, target_fps: f64, set_timecode: bool, timecode: &str, normalize_ebu_r128: bool, mut on_progress: impl FnMut(f64) + Send + 'static) -> Result<()> {
    // Get duration with ffprobe to calculate fraction
    let duration = probe_duration_secs(input).unwrap_or(0.0);

    // Resolve ffmpeg path: 1) /app/bin/ffmpeg 2) next to the executable 3) in the PATH
    let ffmpeg_path = find_ffmpeg_binary();

    let base_cmd = |prog: &str| {
        let mut cmd = Command::new(prog);
        cmd.arg("-y")
        .arg("-i").arg(input)
        .args(["-c:v", "dnxhd"])
        .args(["-profile:v", profile])
        .args(["-pix_fmt", select_pix_fmt(profile)])
        .args(["-c:a", if audio_bits == 24 { "pcm_s24le" } else { "pcm_s16le" }])
        .args(["-ac", &audio_channels.to_string()]);

        if !preserve_fps {
            cmd.args(["-r", &format!("{:.3}", target_fps)]);
        }

        if set_timecode {
            cmd.args(["-timecode", timecode]);
        }

        cmd.arg("-progress").arg("pipe:1")
        .arg("-nostats")
        .arg(output)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
        cmd
    };

    // Normalization EBU R128 (optional)
    if normalize_ebu_r128 {
        if let Some(params) = measure_loudness_params(&ffmpeg_path, input)? {
            // apply second pass with measured_* params
            let mut cmd = base_cmd(&ffmpeg_path);
            let af = format!(
                "loudnorm=I=-23:TP=-2:LRA=7:measured_I={}:measured_LRA={}:measured_TP={}:measured_thresh={}:print_format=summary",
                params.i, params.lra, params.tp, params.thresh
                );
            cmd.arg("-af").arg(af);
            let mut child = cmd.spawn().with_context(|| "Failed to start ffmpeg (normalization)")?;
            attach_progress(&mut child, duration, &mut on_progress)?;
            let status = child.wait()?;
            if !status.success() { anyhow::bail!("ffmpeg returned error code on normalization pass: {:?}", status.code()); }
            return Ok(());
        }
    }

    // No normalization: direct execution
    let mut child = base_cmd(&ffmpeg_path)
    .spawn()
    .with_context(|| format!("Unable to start ffmpeg on {}", ffmpeg_path))?;

    attach_progress(&mut child, duration, &mut on_progress)?;

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("ffmpeg returned error code: {:?}", status.code());
    }
    Ok(())
}

fn attach_progress(child: &mut std::process::Child, duration: f64, on_progress: &mut impl FnMut(f64)) -> Result<()> {
    if let Some(out) = child.stdout.take() {
        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(out);
        let mut line = String::new();
        let mut sent_indeterminate = false;
        let known_duration = duration > 0.0;
        while let Ok(n) = reader.read_line(&mut line) {
            // Log raw progress line for debugging (trim to avoid spam)
            if n > 0 {
                let dbg = line.trim();
                if !dbg.is_empty() {
                    eprintln!("[DEBUG] ffmpeg progress: {}", dbg);
                }
            }
            if n == 0 { break; }
            // Lines formatted: key=value. Note: out_time_ms is microseconds despite the name.
            // Prefer out_time_us if present; fall back to out_time_ms.
            let mut handled_time = false;
            if let Some(rest) = line.strip_prefix("out_time_us=") {
                if let Ok(us) = rest.trim().parse::<u64>() {
                    handled_time = true;
                    if known_duration {
                        let frac = (us as f64 / 1_000_000.0) / duration;
                        eprintln!("[DEBUG] computed fraction {:.3}", frac);
                        on_progress(frac.clamp(0.0, 0.999));
                    } else if !sent_indeterminate {
                        eprintln!("[DEBUG] unknown duration, switching to indeterminate pulsing");
                        on_progress(-1.0);
                        sent_indeterminate = true;
                    }
                }
            }
            if !handled_time {
                if let Some(rest) = line.strip_prefix("out_time_ms=") {
                    if let Ok(us_misnamed) = rest.trim().parse::<u64>() {
                        // Despite the name, this is also microseconds.
                        if known_duration {
                            let frac = (us_misnamed as f64 / 1_000_000.0) / duration;
                            eprintln!("[DEBUG] computed fraction {:.3}", frac);
                            on_progress(frac.clamp(0.0, 0.999));
                        } else if !sent_indeterminate {
                            eprintln!("[DEBUG] unknown duration, switching to indeterminate pulsing");
                            on_progress(-1.0);
                            sent_indeterminate = true;
                        }
                    }
                }
            }
            if line.starts_with("progress=end") {
                eprintln!("[DEBUG] ffmpeg progress=end");
                on_progress(1.0);
            }
            line.clear();
        }
    }

    Ok(())
}

fn select_pix_fmt(profile: &str) -> &'static str {
    match profile {
        "dnxhr_hqx" => "yuv422p10le",
        "dnxhr_444" => "yuv444p10le",
        _ => "yuv422p",
    }
}

fn find_ffmpeg_binary() -> String {
    // Priority: Flatpak (/app/bin/ffmpeg) -> in side of executable -> PATH
    let flatpak_ffmpeg = PathBuf::from("/app/bin/ffmpeg");
    if flatpak_ffmpeg.exists() {
        return flatpak_ffmpeg.to_string_lossy().into_owned();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let local = dir.join("ffmpeg");
            if local.exists() {
                return local.to_string_lossy().into_owned();
            }
        }
    }
    "ffmpeg".to_string()
}

fn probe_duration_secs(input: &Path) -> Option<f64> {
    let ffprobe = find_ffprobe_binary();
    let out = Command::new(ffprobe)
    .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
    .arg(input)
    .output()
    .ok()?;
    if !out.status.success() { return None; }
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim().parse::<f64>().ok()
}

fn find_ffprobe_binary() -> String {
    let flatpak_ffprobe = PathBuf::from("/app/bin/ffprobe");
    if flatpak_ffprobe.exists() {
        return flatpak_ffprobe.to_string_lossy().into_owned();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let local = dir.join("ffprobe");
            if local.exists() {
                return local.to_string_lossy().into_owned();
            }
        }
    }
    "ffprobe".to_string()
}

struct LoudnessParams { i: f64, lra: f64, tp: f64, thresh: f64 }

fn measure_loudness_params(ffmpeg: &str, input: &Path) -> Result<Option<LoudnessParams>> {
    // First pass: just measure, no way out
    let child = Command::new(ffmpeg)
    .arg("-hide_banner")
    .arg("-i").arg(input)
    .arg("-af").arg("loudnorm=I=-23:TP=-2:LRA=7:print_format=json")
    .arg("-f").arg("null").arg("-")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .with_context(|| "Falha ao iniciar ffmpeg para medição EBU R128")?;

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let mut txt = String::new();
    txt.push_str(&String::from_utf8_lossy(&output.stdout));
    txt.push_str(&String::from_utf8_lossy(&output.stderr));

    // Extract JSON block
    if let Some(start) = txt.find('{') {
        if let Some(end) = txt.rfind('}') {
            let json_str = &txt[start..=end];
            // Rudimentary Parse of required fields
            let i = extract_json_number(json_str, "input_i").or_else(|| extract_json_number(json_str, "measured_I")).unwrap_or(-23.0);
            let lra = extract_json_number(json_str, "input_lra").or_else(|| extract_json_number(json_str, "measured_LRA")).unwrap_or(7.0);
            let tp = extract_json_number(json_str, "input_tp").or_else(|| extract_json_number(json_str, "measured_TP")).unwrap_or(-2.0);
            let thresh = extract_json_number(json_str, "input_thresh").or_else(|| extract_json_number(json_str, "measured_thresh")).unwrap_or(-34.0);
            return Ok(Some(LoudnessParams { i, lra, tp, thresh }));
        }
    }
    Ok(None)
}

fn extract_json_number(s: &str, key: &str) -> Option<f64> {
    // Busca "key": valor
    let pat = format!("\"{}\"", key);
    if let Some(pos) = s.find(&pat) {
        let rest = &s[pos + pat.len()..];
        if let Some(colon) = rest.find(':') {
            let rest = &rest[colon + 1..];
            let mut num = String::new();
            for c in rest.chars() {
                if c.is_ascii_digit() || c == '-' || c == '.' {
                    num.push(c);
                } else if !num.is_empty() {
                    break;
                }
            }
            if let Ok(v) = num.parse::<f64>() { return Some(v); }
        }
    }
    None
}
