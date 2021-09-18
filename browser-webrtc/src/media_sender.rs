use async_std::sync::Arc;
use web_sys::{MediaStream, RtcPeerConnection, RtcRtpSender};

use crate::Sender;

#[derive(Debug)]
pub struct MediaSender {
    sender: Arc<Sender>,
    js_connection: RtcPeerConnection,
    js_media_stream: MediaStream,
    js_rtc_rtp_senders: Vec<RtcRtpSender>,
}

impl MediaSender {
    pub fn new(
        sender: Arc<Sender>,
        js_connection: RtcPeerConnection,
        js_media_stream: MediaStream,
    ) -> Arc<Self> {
        log::trace!("browser_webrtc::MediaSender::new");

        use wasm_bindgen::JsCast;
        use web_sys::MediaStreamTrack;

        let tracks = js_media_stream.get_tracks();
        let js_rtc_rtp_senders = tracks
            .iter()
            .map(|track| {
                let track: MediaStreamTrack = track.dyn_into().unwrap();
                js_connection.add_track_0(&track, &js_media_stream)
            })
            .collect();

        Arc::new(Self {
            sender,
            js_connection,
            js_media_stream,
            js_rtc_rtp_senders,
        })
    }

    pub fn media_stream(&self) -> &MediaStream {
        &self.js_media_stream
    }
}

impl Drop for MediaSender {
    fn drop(&mut self) {
        log::trace!("browser_webrtc::MediaSender::drop");

        for sender in self.js_rtc_rtp_senders.iter() {
            self.js_connection.remove_track(&sender);
        }
    }
}
