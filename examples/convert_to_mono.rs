// Hound -- A wav encoding and decoding library in Rust
// Copyright (C) 2026 Ruud van Asseldonk
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// A copy of the License has been included in the root of the repository.
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// This example flattens a multi-channel (e.g. stereo or surround) wav file
// into a mono stream, and outputs that stream to stdout.

// Example outputting to a file:
//  cargo run --example convert_to_mono -- input.wav > mono.wav
// Note that outputting it to a file this way might not play in all media
// players as the duration is not set to be accurate in the header.

// Example playing back directly through MPV:
//  cargo run --example convert_to_mono -- input.wav | mpv -

extern crate hound;

use std::env;
use std::io;
use std::io::Write;

/// Stream samples from a WavReader and write mono samples of the same
/// format to the writer.
///
/// This assumes the wav header has already been written.
fn mux_into_mono<S, R, W>(
    reader: &mut hound::WavReader<R>,
    writer: &mut W,
) -> hound::Result<()>
where
    S: hound::Sample + std::ops::Add<Output=S> + std::ops::Div<Output=S> + std::convert::TryFrom<i16, Error=E>,
    R: io::Read,
    W: io::Write,
{
    let channel_count = reader.spec().channels;
    let bit_depth = reader.spec().bits_per_sample;

    // If you know the channel count ahead of time (and are on nightly) you could
    // consider using `Iterator.array_chunks`.
    //
    // Alternatively, if you're OK with reading the entire wav file in, you could
    // `.collect` into a `Vec` and then use [`Vec::chunks`].
    //
    // In our case, we want to be able to deal with any file, so we must account
    // for an arbitrary amount of channels, as well as stream the output in case
    // the file is very large, so we use a channel-size buffer to store a sample
    // for each channel that we can then average to create a mono sample.
    let mut mono_buffer = Vec::<S>::with_capacity(channel_count as usize);

    for sample in
        reader.samples::<S>()
    {
        let sample = sample?;

        mono_buffer.push(sample);

        if mono_buffer.len() >= channel_count as usize {
            // To prevent overflow in the case of integer samples, we divide first then add
            let mono_sample: S = mono_buffer.drain(..).fold(
                S::try_from(0).unwrap(),
                |acc, x| acc + (x / S::try_from(channel_count as i16)?)
            );
            mono_sample.write(writer, bit_depth)?;
        }
    }

    // Should not really happen if the wav file is well formed, but this will
    // catch any mismatched samples at the end.
    if !mono_buffer.is_empty() {
        let remaining_channels = mono_buffer.len();
        let mono_sample: S = mono_buffer.drain(..).fold(
            S::try_from(0).unwrap(),
            |acc, x| acc + (x / S::try_from(remaining_channels as i16)?)
        );
        mono_sample.write(writer, bit_depth)?;
    }

    Ok(())
}

fn main() -> hound::Result<()> {
    // Open a WavReader using the file provided on the command line.
    let fname = env::args().nth(1).expect("no file given");
    let mut reader = hound::WavReader::open(fname)?;
    let input_spec = reader.spec();

    // The output spec is the same as the input spec, but in mono.
    let output_spec = hound::WavSpec {
        channels: 1,
        ..input_spec
    };

    // Write info to stderr so the result is pipeable.
    eprintln!(
        "Converting {0} channel {1}Hz {2}bit stream of {3} samples to 1 channel {1}Hz {2}bit stream",
        input_spec.channels, input_spec.sample_rate, input_spec.bits_per_sample, reader.duration()
    );

    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    // Write output header for streaming.
    let header = output_spec.into_header_for_infinite_file();
    stdout.write_all(&header[..]).unwrap();

    // Perform calculations in the same format as the sample format.
    match (input_spec.sample_format, input_spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => {
            mux_into_mono::<i16, _, _>(&mut reader, &mut stdout)?;
        }
        (hound::SampleFormat::Int, _) => {
            mux_into_mono::<i32, _, _>(&mut reader, &mut stdout)?;
        }
        (hound::SampleFormat::Float, _) => {
            mux_into_mono::<f32, _, _>(&mut reader, &mut stdout)?;
        }
    }

    Ok(())
}
