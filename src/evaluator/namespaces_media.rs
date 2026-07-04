// namespaces_media.rs — `Media`: reproducción de audio nativa (rodio: wav,
// mp3, flac, vorbis). Sonidos asíncronos identificados por id (int).
//
// Alcance: SOLO audio. Decodificar video real requiere ffmpeg (dependencia
// externa; decisión de diseño pendiente — ver serez-clip); los frames de
// imagen ya entran por Gui.loadImage/loadImageBytes.
//
// Permiso: 'Media' (denegación fatal, como el resto de permisos).
// Errores de runtime (dispositivo ausente, archivo ilegible, formato no
// soportado) → capturables: IOError / MediaError.
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

use crate::ast;
use crate::region::ObjectData;
use super::EvalResult;

pub struct MediaState {
    // El stream debe vivir mientras suene algo (dropearlo corta el audio).
    _stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
    sinks: HashMap<i64, rodio::Sink>,
    next: i64,
}

impl MediaState {
    fn new() -> Option<Self> {
        let (stream, handle) = rodio::OutputStream::try_default().ok()?;
        Some(MediaState { _stream: stream, handle, sinks: HashMap::new(), next: 1 })
    }

    /// Retira los sinks que ya terminaron (no crecen sin límite).
    fn sweep(&mut self) {
        self.sinks.retain(|_, s| !s.empty());
    }
}

impl super::Evaluator {
    pub(super) fn eval_media_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        if !self.permissions.contains("Media") {
            eprintln!(
                "❌ ERROR: 'Media' requires permission 'Media' — declare it in serez.json \
                 (\"permissions\": [\"Media\", ...]) or with `use permissions {{ Media }}`"
            );
            return EvalResult::Error;
        }

        match dot_call.method.as_str() {
            // Media.playSound(path) -> id. Reproduce en segundo plano.
            "playSound" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Media.playSound(path) requires 1 argument");
                }
                let path = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(p) => p,
                    None => { return self.rt_err_kind("TypeError", "Media.playSound path must be a string"); }
                };
                // Archivo primero: así un path malo da IOError aunque la
                // máquina no tenga dispositivo de audio.
                let file = match File::open(&path) {
                    Ok(f) => f,
                    Err(e) => { return self.rt_err_kind("IOError", &format!("Media.playSound: cannot open '{}': {}", path, e)); }
                };
                let decoder = match rodio::Decoder::new(BufReader::new(file)) {
                    Ok(d) => d,
                    Err(e) => { return self.rt_err_kind("MediaError", &format!("Media.playSound: unsupported or corrupt audio '{}': {}", path, e)); }
                };
                if self.media.is_none() {
                    self.media = MediaState::new();
                    if self.media.is_none() {
                        return self.rt_err_kind("MediaError", "Media.playSound: no audio output device available");
                    }
                }
                let media = self.media.as_mut().unwrap();
                media.sweep();
                let sink = match rodio::Sink::try_new(&media.handle) {
                    Ok(s) => s,
                    Err(e) => { return self.rt_err_kind("MediaError", &format!("Media.playSound: cannot create playback sink: {}", e)); }
                };
                sink.append(decoder);
                let id = media.next;
                media.next += 1;
                media.sinks.insert(id, sink);
                EvalResult::Value(self.alloc(ObjectData::Integer(id)))
            }

            // Media.isPlaying(id) -> bool
            "isPlaying" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Media.isPlaying(id) requires 1 argument");
                }
                let id = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) => v,
                    None => { return self.rt_err_kind("TypeError", "Media.isPlaying id must be an integer"); }
                };
                let playing = self.media.as_ref()
                    .and_then(|m| m.sinks.get(&id))
                    .map(|s| !s.empty() && !s.is_paused())
                    .unwrap_or(false);
                EvalResult::Value(if playing { self.true_ref } else { self.false_ref })
            }

            // Media.stop(id) -> bool (true si existía)
            "stop" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Media.stop(id) requires 1 argument");
                }
                let id = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) => v,
                    None => { return self.rt_err_kind("TypeError", "Media.stop id must be an integer"); }
                };
                let existed = self.media.as_mut()
                    .and_then(|m| m.sinks.remove(&id))
                    .map(|s| { s.stop(); true })
                    .unwrap_or(false);
                EvalResult::Value(if existed { self.true_ref } else { self.false_ref })
            }

            "stopAll" => {
                if let Some(m) = self.media.as_mut() {
                    for (_, s) in m.sinks.drain() {
                        s.stop();
                    }
                }
                EvalResult::Value(self.null_ref)
            }

            // Media.pause(id) / Media.resume(id) -> bool
            "pause" | "resume" => {
                let method = dot_call.method.clone();
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", &format!("Media.{}(id) requires 1 argument", method));
                }
                let id = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) => v,
                    None => { return self.rt_err_kind("TypeError", &format!("Media.{} id must be an integer", method)); }
                };
                let ok = self.media.as_ref()
                    .and_then(|m| m.sinks.get(&id))
                    .map(|s| {
                        if method == "pause" { s.pause(); } else { s.play(); }
                        true
                    })
                    .unwrap_or(false);
                EvalResult::Value(if ok { self.true_ref } else { self.false_ref })
            }

            // Media.setVolume(id, vol) — vol 0..200 (100 = normal) -> bool
            "setVolume" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Media.setVolume(id, volume) requires 2 arguments");
                }
                let id = self.gui_int_arg(&dot_call.arguments[0]);
                let vol = self.gui_int_arg(&dot_call.arguments[1]);
                let (id, vol) = match (id, vol) {
                    (Some(i), Some(v)) => (i, v.clamp(0, 200)),
                    _ => { return self.rt_err_kind("TypeError", "Media.setVolume requires (int, int)"); }
                };
                let ok = self.media.as_ref()
                    .and_then(|m| m.sinks.get(&id))
                    .map(|s| { s.set_volume(vol as f32 / 100.0); true })
                    .unwrap_or(false);
                EvalResult::Value(if ok { self.true_ref } else { self.false_ref })
            }

            // Media.playingCount() -> int (sonidos activos)
            "playingCount" => {
                let n = self.media.as_mut()
                    .map(|m| { m.sweep(); m.sinks.len() as i64 })
                    .unwrap_or(0);
                EvalResult::Value(self.alloc(ObjectData::Integer(n)))
            }

            other => {
                let o = other.to_string();
                self.rt_err_kind("TypeError", &format!("Media.{}: unknown method", o))
            }
        }
    }
}
