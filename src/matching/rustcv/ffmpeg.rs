use console::style;
use futures::stream::StreamExt;
use image::ImageBuffer;
use snafu::{OptionExt, ResultExt};
use std::process::Stdio;
use tokio::codec::FramedRead;
use tokio_process::Command;

use crate::codec::{FrameBuffer, VideoFrameCodec};
use crate::error::*;
use crate::ffmpeg::{get_video_dimensions, get_video_duration};
use crate::pixel::{get_blended_col_average_pixels, get_simple_col_average_pixels, Pixel};
use crate::progress::RipperBar;

#[derive(Copy, Clone)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

impl Dimensions {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

pub struct FrameRipper<'a> {
    input_path: &'a str,
    output_path: &'a str,
    is_simple: bool,
}

impl<'a> FrameRipper<'a> {
    pub fn new(input_path: &'a str, output_path: &'a str, is_simple: bool) -> Self {
        Self {
            input_path,
            output_path,
            is_simple,
        }
    }

    pub async fn rip(&mut self) -> Result<()> {
        let duration = get_video_duration(self.input_path)?;
        let duration = f64::floor(duration - 1.0);
        let video_dimensions = &get_video_dimensions(self.input_path)?;
        let aspect_preserved_width = video_dimensions.height * 3;
        let barcode_dimensions = Dimensions::new(aspect_preserved_width, video_dimensions.height);
        let pixels = self
            .spawn_ffmpeg_ripper(duration, &video_dimensions, &barcode_dimensions)
            .await?;
        self.save_barcode(pixels, &barcode_dimensions)?;
        Ok(())
    }

    async fn spawn_ffmpeg_ripper(
        &self,
        duration: f64,
        video_dimensions: &Dimensions,
        barcode_dimensions: &Dimensions,
    ) -> Result<Vec<Pixel>> {
        let fps_dividend = duration / barcode_dimensions.width as f64;

        let mut cmd = Command::new("ffmpeg");
        cmd.args(&[
            "-i",
            self.input_path,
            "-vf",
            &format!("fps=1/{:?}", fps_dividend),
            "-f",
            "image2pipe",
            "-pix_fmt",
            "rgb24",
            "-vcodec",
            "rawvideo",
            "-",
        ]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());
        let mut child = cmd.spawn().context(CommandSpawnError)?;

        let stdout = child.stdout().take().context(StdoutHandleError)?;

        let mut reader = FramedRead::new(
            stdout,
            VideoFrameCodec::new(video_dimensions.width, video_dimensions.height),
        );

        tokio::spawn(async {
            // TODO: How to make snafu errors work inside of a tokio executor with FfmepgError?
            let status = child.await.expect("Child process encountered an error.");

            println!("child status was: {}", status);
        });

        let mut pixels = Vec::with_capacity(barcode_dimensions.width as usize);
        let progress_bar = RipperBar::new(self.input_path, barcode_dimensions);
        progress_bar.print_prelude();
        while let Some(Ok(bytes_mut_buffer)) = reader.next().await {
            let frame_buffer = FrameBuffer::from_raw(
                video_dimensions.width,
                video_dimensions.height,
                bytes_mut_buffer.to_vec(),
            )
            .context(FrameBufferError)?;
            let mut average_pixels = match self.is_simple {
                true => get_simple_col_average_pixels(frame_buffer, video_dimensions),
                false => get_blended_col_average_pixels(frame_buffer, video_dimensions),
            };
            pixels.append(average_pixels.as_mut());
            progress_bar.bar.inc(1);
        }

        Ok(pixels)
    }

    fn save_barcode(&self, average_pixels: Vec<Pixel>, dimensions: &Dimensions) -> Result<()> {
        let img = ImageBuffer::from_fn(dimensions.width, dimensions.height as u32, |row, col| {
            average_pixels[(row * dimensions.height as u32 + col) as usize]
        });
        img.save(self.output_path).context(BarcodeSaveError)?;

        let success_msg = format!(
            "\n{} {}",
            style("Successfully wrote barcode to ").green(),
            style(self.output_path).green()
        );

        eprintln!("{}", success_msg);

        Ok(())
    }
}
