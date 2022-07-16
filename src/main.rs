use std::fmt::Write;
use std::time::Duration;
use std::sync::Mutex;
use mpd::Client;
use chrono::{DateTime, Local};

#[link(name = "X11")]
extern {
    fn XOpenDisplay(screen: usize) -> usize;
    fn XStoreName(display: usize, window: usize, name: *const u8) -> i32;
    fn XDefaultRootWindow(display: usize) -> usize;
    fn XFlush(display: usize) -> i32;
}

fn main() {
    // Song info
    let song_info = Mutex::new(String::new());

    std::thread::scope(|x| {
        // MPD status thread
        x.spawn(|| {
            let mut mpd = Client::connect("127.0.0.1:6600");

            loop {
                // If we didn't connect to MPD or MPD is not giving us a
                // status, assume we need to reconnect
                if mpd.is_err() || mpd.as_mut().ok()
                        .and_then(|x| x.status().ok()).is_none() {
                    mpd = Client::connect("127.0.0.1:6600");
                }

                if mpd.as_mut().ok().and_then(|mpd| {
                    let cs = mpd.currentsong().ok().flatten();
                    let status = mpd.status().ok();

                    let mut song_info = song_info.lock().unwrap();
                    song_info.clear();

                    cs.map(|song| {
                        write!(song_info, "{} - {} - {} ({})",
                            song.title.as_ref().map(|x| x.as_str())
                                .unwrap_or("Unknown Song"),
                            song.tags.get("Artist")
                                .as_ref().map(|x| x.as_str())
                                .unwrap_or("Unknown Artist"),
                            song.tags.get("Album").as_ref().map(|x| x.as_str())
                                .unwrap_or("Unknown Album"),
                            song.tags.get("Date").as_ref().map(|x| x.as_str())
                                .unwrap_or("????")).unwrap();
                    }).and_then(|_| {
                        status.map(|status| {
                            let elapsed = status.elapsed
                                .unwrap_or(chrono::Duration::zero());
                            let duration = status.duration
                                .unwrap_or(chrono::Duration::zero());

                            write!(song_info, " [{:02}:{:02} - {:02}:{:02}]",
                                elapsed.num_minutes(),
                                elapsed.num_seconds() % 60,
                                duration.num_minutes(),
                                duration.num_seconds() % 60).unwrap();
                        })
                    })
                }).is_none() {
                    let mut song_info = song_info.lock().unwrap();
                    song_info.clear();
                    write!(song_info, "No song playing").unwrap();
                }

                std::thread::sleep(Duration::from_nanos((1e9 / 10.) as u64));
            }
        });

        // X updater thread
        x.spawn(|| {
            // Connect to X
            let disp = unsafe { XOpenDisplay(0) };
            let root = unsafe { XDefaultRootWindow(disp) };

            // Status string
            let mut status = String::new();

            loop {
                // Clear status
                status.clear();

                // Get the time and make the status message
                let local: DateTime<Local> = Local::now();
                let song_info = song_info.lock().unwrap();
                write!(status, "{} - {}\0",
                    song_info, local.format("%F %T.%3f")).unwrap();
                drop(song_info);

                // Write and flush the status
                unsafe { XStoreName(disp, root, status.as_ptr()); }
                unsafe { XFlush(disp); }

                std::thread::sleep(Duration::from_nanos((1e9 / 144.) as u64));
            }
        });
    });
}

