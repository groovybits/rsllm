use crate::hexdump;
use crate::stream_data::StreamData;
use h264_reader::annexb::AnnexBReader;
use h264_reader::nal::{pps, sei, slice, sps, Nal, RefNal, UnitType};
use h264_reader::push::NalInterest;
use h264_reader::Context;
use hex_slice::AsHex;
use log::{debug, error, info};
use mpeg2ts_reader::demultiplex;
use mpeg2ts_reader::packet;
use mpeg2ts_reader::packet::Pid;
use mpeg2ts_reader::pes;
use mpeg2ts_reader::psi;
use mpeg2ts_reader::StreamType;
use scte35_reader;
use std::cell;
use std::cmp;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{self};
use tokio::task;
use tokio::time::Duration;

const DEBUG_PTS: bool = true;
const DEBUG_PAYLOAD: bool = false;
const DEBUG_PES: bool = true;
const DEBUG_PCR: bool = true;
const DEBUG_SCTE35: bool = true;

fn is_cea_608(itu_t_t35_data: &sei::user_data_registered_itu_t_t35::ItuTT35) -> bool {
    // In this example, we check if the ITU-T T.35 data matches the known format for CEA-608.
    // This is a simplified example and might need adjustment based on the actual data format.
    match itu_t_t35_data {
        sei::user_data_registered_itu_t_t35::ItuTT35::UnitedStates => true,
        _ => false,
    }
}

// This function checks if the byte is a standard ASCII character
fn is_standard_ascii(byte: u8) -> bool {
    byte >= 0x20 && byte <= 0x7F
}

// Function to check if the byte pair represents XDS data
fn is_xds(byte1: u8, byte2: u8) -> bool {
    // Implement logic to identify XDS data
    // Placeholder logic: Example only
    byte1 == 0x01 && byte2 >= 0x20 && byte2 <= 0x7F
}

// Function to decode CEA-608 CC1/CC2
fn decode_cea_608_cc1_cc2(byte1: u8, byte2: u8) -> Option<String> {
    decode_character(byte1, byte2)
    // The above line replaces the previous implementation and uses decode_character
    // to handle both ASCII and control codes.
}

fn decode_cea_608_xds(byte1: u8, byte2: u8) -> Option<String> {
    if is_xds(byte1, byte2) {
        Some(format!("XDS: {:02X} {:02X}", byte1, byte2))
    } else {
        None
    }
}

// Decode CEA-608 characters, including control codes
fn decode_character(byte1: u8, byte2: u8) -> Option<String> {
    debug!("Decoding: {:02X} {:02X}", byte1, byte2); // Debugging

    // Handle standard ASCII characters
    if is_standard_ascii(byte1) && is_standard_ascii(byte2) {
        return Some(format!("{}{}", byte1 as char, byte2 as char));
    }

    // Handle special control characters (Example)
    // This is a simplified version, actual implementation may vary based on control characters
    match (byte1, byte2) {
        (0x14, 0x2C) => Some(String::from("[Clear Caption]")),
        (0x14, 0x20) => Some(String::from("[Roll-Up Caption]")),
        // Add more control character handling here
        _ => {
            error!("Unhandled control character: {:02X} {:02X}", byte1, byte2); // Debugging
            None
        }
    }
}

// Simplified CEA-608 decoding function
// Main CEA-608 decoding function
fn decode_cea_608(data: &[u8]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut captions_cc1 = Vec::new();
    let mut captions_cc2 = Vec::new();
    let mut xds_data = Vec::new();

    for chunk in data.chunks(3) {
        if chunk.len() == 3 {
            match chunk[0] {
                0x04 => {
                    if let Some(decoded) = decode_cea_608_cc1_cc2(chunk[1], chunk[2]) {
                        captions_cc1.push(decoded);
                    } else if let Some(decoded) = decode_cea_608_xds(chunk[1], chunk[2]) {
                        xds_data.push(decoded);
                    }
                }
                0x05 => {
                    if let Some(decoded) = decode_cea_608_cc1_cc2(chunk[1], chunk[2]) {
                        captions_cc2.push(decoded);
                    }
                }
                _ => debug!("Unknown caption channel: {:02X}", chunk[0]),
            }
        }
    }

    (captions_cc1, captions_cc2, xds_data)
}

pub struct DumpSpliceInfoProcessor {
    pub elementary_pid: Option<Pid>,
    pub last_pcr: Rc<cell::Cell<Option<packet::ClockRef>>>,
}
impl scte35_reader::SpliceInfoProcessor for DumpSpliceInfoProcessor {
    fn process(
        &self,
        header: scte35_reader::SpliceInfoHeader<'_>,
        command: scte35_reader::SpliceCommand,
        descriptors: scte35_reader::SpliceDescriptors<'_>,
    ) {
        if DEBUG_SCTE35 {
            if let Some(elementary_pid) = self.elementary_pid {
                print!("{:?} ", elementary_pid);
            }
            if let Some(pcr) = self.last_pcr.as_ref().get() {
                print!("Last {:?}: ", pcr)
            }
            print!("{:?} {:#?}", header, command);
        }
        if let scte35_reader::SpliceCommand::SpliceInsert { splice_detail, .. } = command {
            if let scte35_reader::SpliceInsert::Insert { splice_mode, .. } = splice_detail {
                if let scte35_reader::SpliceMode::Program(scte35_reader::SpliceTime::Timed(t)) =
                    splice_mode
                {
                    if let Some(time) = t {
                        let time_ref = mpeg2ts_reader::packet::ClockRef::from_parts(time, 0);
                        if let Some(pcr) = self.last_pcr.as_ref().get() {
                            let mut diff = time_ref.base() as i64 - pcr.base() as i64;
                            if diff < 0 {
                                diff += (std::u64::MAX / 2) as i64;
                            }
                            if DEBUG_SCTE35 {
                                print!(" {}ms after last PCR", diff / 90);
                            }
                        }
                    }
                }
            }
        }
        if DEBUG_SCTE35 {
            println!();
        }
        for d in &descriptors {
            if DEBUG_SCTE35 {
                println!(" - {:#?}", d);
            }
        }
    }
}

pub struct Scte35StreamConsumer {
    section: psi::SectionPacketConsumer<
        psi::CompactSyntaxSectionProcessor<
            psi::BufferCompactSyntaxParser<
                scte35_reader::Scte35SectionProcessor<DumpSpliceInfoProcessor, DumpDemuxContext>,
            >,
        >,
    >,
}

impl Scte35StreamConsumer {
    fn new(elementary_pid: Pid, last_pcr: Rc<cell::Cell<Option<packet::ClockRef>>>) -> Self {
        let parser = scte35_reader::Scte35SectionProcessor::new(DumpSpliceInfoProcessor {
            elementary_pid: Some(elementary_pid),
            last_pcr,
        });
        Scte35StreamConsumer {
            section: psi::SectionPacketConsumer::new(psi::CompactSyntaxSectionProcessor::new(
                psi::BufferCompactSyntaxParser::new(parser),
            )),
        }
    }

    fn construct(
        last_pcr: Rc<cell::Cell<Option<packet::ClockRef>>>,
        program_pid: packet::Pid,
        pmt: &psi::pmt::PmtSection<'_>,
        stream_info: &psi::pmt::StreamInfo<'_>,
    ) -> DumpFilterSwitch {
        if scte35_reader::is_scte35(pmt) {
            if DEBUG_SCTE35 {
                info!(
                    "Program {:?}: {:?} has type {:?}, but PMT has 'CUEI' registration_descriptor that would indicate SCTE-35 content",
                    program_pid,
                    stream_info.elementary_pid(),
                    stream_info.stream_type()
                );
            }
            DumpFilterSwitch::Scte35(Scte35StreamConsumer::new(
                stream_info.elementary_pid(),
                last_pcr,
            ))
        } else {
            if DEBUG_SCTE35 {
                info!(
                    "Program {:?}: {:?} has type {:?}, but PMT lacks 'CUEI' registration_descriptor that would indicate SCTE-35 content",
                    program_pid,
                    stream_info.elementary_pid(),
                    stream_info.stream_type()
                );
            }
            DumpFilterSwitch::Null(demultiplex::NullPacketFilter::default())
        }
    }
}
impl demultiplex::PacketFilter for Scte35StreamConsumer {
    type Ctx = DumpDemuxContext;
    fn consume(&mut self, ctx: &mut Self::Ctx, pk: &packet::Packet<'_>) {
        self.section.consume(ctx, pk);
    }
}

pub struct PcrWatch(Rc<cell::Cell<Option<packet::ClockRef>>>);
impl demultiplex::PacketFilter for PcrWatch {
    type Ctx = DumpDemuxContext;
    fn consume(&mut self, _ctx: &mut Self::Ctx, pk: &packet::Packet<'_>) {
        if let Some(af) = pk.adaptation_field() {
            if let Ok(pcr) = af.pcr() {
                self.0.set(Some(pcr));
                if DEBUG_PCR {
                    info!("Got PCR: {:?}", pcr);
                }
            }
        }
    }
}

mpeg2ts_reader::packet_filter_switch! {
    DumpFilterSwitch<DumpDemuxContext> {
        Pat: demultiplex::PatPacketFilter<DumpDemuxContext>,
        Pes: pes::PesPacketFilter<DumpDemuxContext,PtsDumpElementaryStreamConsumer>,
        Pmt: demultiplex::PmtPacketFilter<DumpDemuxContext>,
        Null: demultiplex::NullPacketFilter<DumpDemuxContext>,
        Scte35: Scte35StreamConsumer,
        Pcr: PcrWatch,
    }
}
pub struct DumpDemuxContext {
    changeset: demultiplex::FilterChangeset<DumpFilterSwitch>,
    last_pcrs: HashMap<packet::Pid, Rc<cell::Cell<Option<packet::ClockRef>>>>,
}
impl DumpDemuxContext {
    pub fn new() -> Self {
        DumpDemuxContext {
            changeset: demultiplex::FilterChangeset::default(),
            last_pcrs: HashMap::new(),
        }
    }
    pub fn last_pcr(&self, program_pid: packet::Pid) -> Rc<cell::Cell<Option<packet::ClockRef>>> {
        self.last_pcrs
            .get(&program_pid)
            .expect("last_pcrs entry didn't exist on call to last_pcr()")
            .clone()
    }
}
impl demultiplex::DemuxContext for DumpDemuxContext {
    type F = DumpFilterSwitch;

    fn filter_changeset(&mut self) -> &mut demultiplex::FilterChangeset<Self::F> {
        &mut self.changeset
    }

    fn construct(&mut self, req: demultiplex::FilterRequest<'_, '_>) -> Self::F {
        match req {
            demultiplex::FilterRequest::ByPid(packet::Pid::PAT) => {
                DumpFilterSwitch::Pat(demultiplex::PatPacketFilter::default())
            }
            // 'Stuffing' data on PID 0x1fff may be used to pad-out parts of the transport stream
            // so that it has constant overall bitrate.  This causes it to be ignored if present.
            demultiplex::FilterRequest::ByPid(mpeg2ts_reader::STUFFING_PID) => {
                DumpFilterSwitch::Null(demultiplex::NullPacketFilter::default())
            }
            // This match-arm installs our application-specific handling for each H264 stream
            // discovered within the transport stream,
            demultiplex::FilterRequest::ByStream {
                stream_type: StreamType::H264,
                pmt,
                stream_info,
                ..
            } => PtsDumpElementaryStreamConsumer::construct(pmt, stream_info),
            demultiplex::FilterRequest::ByStream {
                program_pid,
                stream_type: scte35_reader::SCTE35_STREAM_TYPE,
                pmt,
                stream_info,
            } => Scte35StreamConsumer::construct(
                self.last_pcr(program_pid),
                program_pid,
                pmt,
                stream_info,
            ),
            demultiplex::FilterRequest::ByStream { program_pid, .. } => {
                DumpFilterSwitch::Pcr(PcrWatch(self.last_pcr(program_pid)))
            }
            demultiplex::FilterRequest::Pmt {
                pid,
                program_number,
            } => {
                // prepare structure needed to print PCR values later on
                self.last_pcrs.insert(pid, Rc::new(cell::Cell::new(None)));
                DumpFilterSwitch::Pmt(demultiplex::PmtPacketFilter::new(pid, program_number))
            }
            demultiplex::FilterRequest::Nit { .. } => {
                DumpFilterSwitch::Null(demultiplex::NullPacketFilter::default())
            }
            demultiplex::FilterRequest::ByPid(_) => {
                DumpFilterSwitch::Null(demultiplex::NullPacketFilter::default())
            }
        }
    }
}

// Implement the ElementaryStreamConsumer to just dump and PTS/DTS timestamps to stdout
pub struct PtsDumpElementaryStreamConsumer {
    pid: packet::Pid,
    len: Option<usize>,
}
impl PtsDumpElementaryStreamConsumer {
    fn construct(
        _pmt_sect: &psi::pmt::PmtSection,
        stream_info: &psi::pmt::StreamInfo,
    ) -> DumpFilterSwitch {
        let filter = pes::PesPacketFilter::new(PtsDumpElementaryStreamConsumer {
            pid: stream_info.elementary_pid(),
            len: None,
        });
        DumpFilterSwitch::Pes(filter)
    }
}
impl pes::ElementaryStreamConsumer<DumpDemuxContext> for PtsDumpElementaryStreamConsumer {
    fn start_stream(&mut self, _ctx: &mut DumpDemuxContext) {}
    fn begin_packet(&mut self, _ctx: &mut DumpDemuxContext, header: pes::PesHeader) {
        match header.contents() {
            pes::PesContents::Parsed(Some(parsed)) => {
                if DEBUG_PTS {
                    match parsed.pts_dts() {
                        Ok(pes::PtsDts::PtsOnly(Ok(pts))) => {
                            print!("{:?}: pts {:#08x}                ", self.pid, pts.value())
                        }
                        Ok(pes::PtsDts::Both {
                            pts: Ok(pts),
                            dts: Ok(dts),
                        }) => print!(
                            "{:?}: pts {:#08x} dts {:#08x} ",
                            self.pid,
                            pts.value(),
                            dts.value()
                        ),
                        _ => (),
                    }
                }
                let payload = parsed.payload();
                self.len = Some(payload.len());
                if DEBUG_PAYLOAD {
                    println!(
                        "{:02x}",
                        payload[..cmp::min(payload.len(), 16)].plain_hex(false)
                    )
                } else if DEBUG_PTS {
                    println!()
                }
            }
            pes::PesContents::Parsed(None) => (),
            pes::PesContents::Payload(payload) => {
                self.len = Some(payload.len());
                if DEBUG_PES {
                    println!(
                        "{:?}:                               {:02x}",
                        self.pid,
                        payload[..cmp::min(payload.len(), 16)].plain_hex(false)
                    )
                }
            }
        }
    }
    fn continue_packet(&mut self, _ctx: &mut DumpDemuxContext, data: &[u8]) {
        if DEBUG_PAYLOAD {
            println!(
                "{:?}:                     continues {:02x}",
                self.pid,
                data[..cmp::min(data.len(), 16)].plain_hex(false)
            )
        }
        self.len = self.len.map(|l| l + data.len());
    }
    fn end_packet(&mut self, _ctx: &mut DumpDemuxContext) {
        if DEBUG_PAYLOAD {
            println!("{:?}: end of packet length={:?}", self.pid, self.len);
        }
    }
    fn continuity_error(&mut self, _ctx: &mut DumpDemuxContext) {}
}

pub fn reader_thread(debug_nal_types: String, debug_nals: bool) {
    let demuxer_channel_size = 10000;
    let decoder_channel_size = 10000;
    let mut ctx = Context::default();
    let mut scratch = Vec::new();
    let running = Arc::new(AtomicBool::new(true));
    let running_decoder = running.clone();
    let running_demuxer = running.clone();
    // Setup demuxer async processing thread
    let (_dtx, mut drx) = mpsc::channel::<Vec<StreamData>>(decoder_channel_size);
    let (dmtx, _dmrx) = mpsc::channel::<Vec<u8>>(demuxer_channel_size);
    // Setup asynchronous demuxer processing thread
    let (_sync_dmtx, mut sync_dmrx) = mpsc::channel::<Vec<u8>>(demuxer_channel_size);
    let parse_short_nals = true;
    let decode_video = true;
    let mpegts_reader = true;
    let packet_size = 188;

    // Use the `move` keyword to move ownership of `ctx` and `scratch` into the closure
    let mut annexb_reader = AnnexBReader::accumulate(move |nal: RefNal<'_>| {
        if !nal.is_complete() {
            return NalInterest::Buffer;
        }
        let hdr = match nal.header() {
            Ok(h) => h,
            Err(e) => {
                // check if we are in debug mode for nals, else check if this is a ForbiddenZeroBit error, which we ignore
                let e_str = format!("{:?}", e);
                if !debug_nals && e_str == "ForbiddenZeroBit" {
                    // ignore forbidden zero bit error unless we are in debug mode
                } else {
                    // show nal contents
                    debug!("---\n{:?}\n---", nal);
                    error!("Failed to parse NAL header: {:?}", e);
                }
                return NalInterest::Buffer;
            }
        };
        match hdr.nal_unit_type() {
            UnitType::SeqParameterSet => {
                if let Ok(sps) = sps::SeqParameterSet::from_bits(nal.rbsp_bits()) {
                    // check if debug_nal_types has sps
                    if debug_nal_types.contains(&"sps".to_string())
                        || debug_nal_types.contains(&"all".to_string())
                    {
                        println!("Found SPS: {:?}", sps);
                    }
                    ctx.put_seq_param_set(sps);
                }
            }
            UnitType::PicParameterSet => {
                if let Ok(pps) = pps::PicParameterSet::from_bits(&ctx, nal.rbsp_bits()) {
                    // check if debug_nal_types has pps
                    if debug_nal_types.contains(&"pps".to_string())
                        || debug_nal_types.contains(&"all".to_string())
                    {
                        println!("Found PPS: {:?}", pps);
                    }
                    ctx.put_pic_param_set(pps);
                }
            }
            UnitType::SEI => {
                let mut r = sei::SeiReader::from_rbsp_bytes(nal.rbsp_bytes(), &mut scratch);
                while let Ok(Some(msg)) = r.next() {
                    match msg.payload_type {
                        sei::HeaderType::PicTiming => {
                            let sps = match ctx.sps().next() {
                                Some(s) => s,
                                None => continue,
                            };
                            let pic_timing = sei::pic_timing::PicTiming::read(sps, &msg);
                            match pic_timing {
                                Ok(pic_timing_data) => {
                                    // Check if debug_nal_types has pic_timing or all
                                    if debug_nal_types.contains(&"pic_timing".to_string())
                                        || debug_nal_types.contains(&"all".to_string())
                                    {
                                        println!("Found PicTiming: {:?}", pic_timing_data);
                                    }
                                }
                                Err(e) => {
                                    error!("Error parsing PicTiming SEI: {:?}", e);
                                }
                            }
                        }
                        h264_reader::nal::sei::HeaderType::BufferingPeriod => {
                            let sps = match ctx.sps().next() {
                                Some(s) => s,
                                None => continue,
                            };
                            let buffering_period =
                                sei::buffering_period::BufferingPeriod::read(&ctx, &msg);
                            // check if debug_nal_types has buffering_period
                            if debug_nal_types.contains(&"buffering_period".to_string())
                                || debug_nal_types.contains(&"all".to_string())
                            {
                                println!(
                                    "Found BufferingPeriod: {:?} Payload: [{:?}] - {:?}",
                                    buffering_period, msg.payload, sps
                                );
                            }
                        }
                        h264_reader::nal::sei::HeaderType::UserDataRegisteredItuTT35 => {
                            match sei::user_data_registered_itu_t_t35::ItuTT35::read(&msg) {
                                Ok((itu_t_t35_data, remaining_data)) => {
                                    if debug_nal_types
                                        .contains(&"user_data_registered_itu_tt35".to_string())
                                        || debug_nal_types.contains(&"all".to_string())
                                    {
                                        println!("Found UserDataRegisteredItuTT35: {:?}, Remaining Data: {:?}", itu_t_t35_data, remaining_data);
                                    }
                                    if is_cea_608(&itu_t_t35_data) {
                                        let (captions_cc1, captions_cc2, xds_data) =
                                            decode_cea_608(remaining_data);
                                        debug!(
                                            "CEA-608 Data: {:?} cc1: {:?} cc2: {:?} xds: {:?}",
                                            itu_t_t35_data, captions_cc1, captions_cc2, xds_data
                                        );
                                        if !captions_cc1.is_empty() {
                                            debug!("CEA-608 CC1 Captions: {:?}", captions_cc1);
                                        }
                                        if !captions_cc2.is_empty() {
                                            debug!("CEA-608 CC2 Captions: {:?}", captions_cc2);
                                        }
                                        if !xds_data.is_empty() {
                                            debug!("CEA-608 XDS Data: {:?}", xds_data);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error parsing ITU T.35 data: {:?}", e);
                                }
                            }
                        }
                        h264_reader::nal::sei::HeaderType::UserDataUnregistered => {
                            // Check if debug_nal_types has user_data_unregistered or all
                            if debug_nal_types.contains(&"user_data_unregistered".to_string())
                                || debug_nal_types.contains(&"all".to_string())
                            {
                                println!(
                                    "Found SEI type UserDataUnregistered {:?} payload: [{:?}]",
                                    msg.payload_type, msg.payload
                                );
                            }
                        }
                        _ => {
                            // check if debug_nal_types has sei
                            if debug_nal_types.contains(&"sei".to_string())
                                || debug_nal_types.contains(&"all".to_string())
                            {
                                println!(
                                    "Unknown Found SEI type {:?} payload: [{:?}]",
                                    msg.payload_type, msg.payload
                                );
                            }
                        }
                    }
                }
            }
            UnitType::SliceLayerWithoutPartitioningIdr
            | UnitType::SliceLayerWithoutPartitioningNonIdr => {
                let msg = slice::SliceHeader::from_bits(&ctx, &mut nal.rbsp_bits(), hdr);
                // check if debug_nal_types has slice
                if debug_nal_types.contains(&"slice".to_string())
                    || debug_nal_types.contains(&"all".to_string())
                {
                    println!("Found NAL Slice: {:?}", msg);
                }
            }
            _ => {
                // check if debug_nal_types has nal
                if debug_nal_types.contains(&"unknown".to_string())
                    || debug_nal_types.contains(&"all".to_string())
                {
                    println!("Found Unknown NAL: {:?}", nal);
                }
            }
        }
        NalInterest::Buffer
    });

    // Running a synchronous task in the background
    let running_demuxer_clone = running_demuxer.clone();
    task::spawn_blocking(move || {
        let mut demux_ctx = DumpDemuxContext::new();
        let mut demux = demultiplex::Demultiplex::new(&mut demux_ctx);
        let mut demux_buf = [0u8; 1880 * 1024];
        let mut buf_end = 0;

        while running_demuxer_clone.load(Ordering::SeqCst) {
            match sync_dmrx.blocking_recv() {
                Some(packet) => {
                    let packet_len = packet.len();
                    let space_left = demux_buf.len() - buf_end;

                    if space_left < packet_len {
                        buf_end = 0; // Reset buffer on overflow
                    }

                    demux_buf[buf_end..buf_end + packet_len].copy_from_slice(&packet);
                    buf_end += packet_len;

                    /*info!("Demuxer push packet of size: {}", packet_len);
                    let packet_arc = Arc::new(packet);
                    hexdump(&packet_arc, 0, packet_len);*/
                    demux.push(&mut demux_ctx, &demux_buf[0..buf_end]);
                    // Additional processing as required
                }
                None => {
                    // Handle error or shutdown
                    break;
                }
            }
        }
    });

    // Spawn a new thread for Decoder communication
    let _decoder_thread = tokio::spawn(async move {
        loop {
            if !running_decoder.load(Ordering::SeqCst) {
                debug!("Decoder thread received stop signal.");
                break;
            }

            if !mpegts_reader && !decode_video {
                // Sleep for a short duration to prevent a tight loop
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            // Use tokio::select to simultaneously wait for a new batch or a stop signal
            tokio::select! {
                Some(mut batch) = drx.recv() => {
                    debug!("Processing {} video packets in decoder thread", batch.len());
                    for stream_data in &batch {
                        // packet is a subset of the original packet, starting at the payload
                        let packet_start = stream_data.packet_start;
                        let packet_end = stream_data.packet_start + stream_data.packet_len;

                        if packet_end - packet_start > packet_size {
                            error!("NAL Parser: Packet size {} is larger than packet buffer size {}. Skipping packet.",
                                packet_end - packet_start, packet_size);
                            continue;
                        }

                        // check if packet_start + 4 is less than packet_end
                        if packet_start + 4 >= packet_end {
                            error!("NAL Parser: Packet size {} {} - {} is less than 4 bytes. Skipping packet.",
                                packet_end - packet_start, packet_start, packet_end);
                            continue;
                        }

                        if mpegts_reader {
                            // Send packet data to the synchronous processing thread
                            dmtx.send(stream_data.packet[packet_start..packet_end].to_vec()).await.unwrap();

                            // check if we are decoding video
                            if !decode_video {
                                continue;
                            }
                        }

                        // Skip MPEG-TS header and adaptation field
                        let header_len = 4;
                        let adaptation_field_control = (stream_data.packet[packet_start + 3] & 0b00110000) >> 4;

                        if adaptation_field_control == 0b10 {
                            continue; // Skip packets with only adaptation field (no payload)
                        }

                        let payload_start = if adaptation_field_control != 0b01 {
                            header_len + 1 + stream_data.packet[packet_start + 4] as usize
                        } else {
                            header_len
                        };

                        // confirm payload_start is sane
                        if payload_start >= packet_end || packet_end - payload_start < 4 {
                            error!("NAL Parser: Payload start {} is invalid with packet_start as {} and packet_end as {}. Skipping packet.",
                                payload_start, packet_start, packet_end);
                            continue;
                        } else {
                            debug!("NAL Parser: Payload start {} is valid with packet_start as {} and packet_end as {}.",
                                payload_start, packet_start, packet_end);
                        }

                        // Process payload, skipping padding bytes
                        let mut pos = payload_start;
                        while pos + 4 < packet_end {
                            if parse_short_nals && stream_data.packet[pos..pos + 3] == [0x00, 0x00, 0x01] {
                                let nal_start = pos;
                                pos += 3; // Move past the short start code

                                // Search for the next start code
                                while pos + 4 <= packet_end &&
                                      stream_data.packet[pos..pos + 4] != [0x00, 0x00, 0x00, 0x01] {
                                    // Check for short start code, 0xff padding, or 0x00000000 sequence
                                    if stream_data.packet[pos..pos + 3] == [0x00, 0x00, 0x01] && pos > nal_start + 3 {
                                        // Found a short start code, so back up and process the NAL unit
                                        break;
                                    } else if stream_data.packet[pos + 1] == 0xff && pos > nal_start + 3 {
                                        // check for 0xff padding and that we are at least 2 bytes into the nal
                                        break;
                                    } else if stream_data.packet[pos..pos + 3] == [0x00, 0x00, 0x00] && pos > nal_start + 3 {
                                        // check for 0x00 0x00 0x00 0x00 sequence to stop at
                                        break;
                                    }
                                    pos += 1;
                                }

                                // check if we only have 4 bytes left in the packet, if so then collect them too
                                if pos + 4 >= packet_end {
                                    while pos < packet_end {
                                        if stream_data.packet[pos..pos + 1] == [0xff] {
                                            // check for 0xff padding and that we are at least 2 bytes into the nal
                                            break;
                                        } else if pos + 2 < packet_end && stream_data.packet[pos..pos + 2] == [0x00, 0x00] {
                                            // check for 0x00 0x00 sequence to stop at
                                            break;
                                        }
                                        pos += 1;
                                    }
                                }

                                let nal_end = pos; // End of NAL unit found or end of packet
                                if nal_end - nal_start > 3 { // Threshold for significant NAL unit size
                                    let nal_unit = &stream_data.packet[nal_start..nal_end];

                                    // Debug print the NAL unit
                                    if debug_nals {
                                        let packet_len = nal_end - nal_start;
                                        info!("Extracted {} byte Short NAL Unit from packet range {}-{}:", packet_len, nal_start, nal_end);
                                        let nal_unit_arc = Arc::new(nal_unit.to_vec());
                                        hexdump(&nal_unit_arc, 0, packet_len);
                                    }

                                    // Process the NAL unit
                                    annexb_reader.push(nal_unit);
                                    annexb_reader.reset();
                                }
                            } else if pos + 4 < packet_end && stream_data.packet[pos..pos + 4] == [0x00, 0x00, 0x00, 0x01] {
                                let nal_start = pos;
                                pos += 4; // Move past the long start code

                                // Search for the next start code
                                while pos + 4 <= packet_end &&
                                      stream_data.packet[pos..pos + 4] != [0x00, 0x00, 0x00, 0x01] {
                                    // Check for short start code
                                    if stream_data.packet[pos..pos + 3] == [0x00, 0x00, 0x01] && pos > nal_start + 3 {
                                        // Found a short start code, so back up and process the NAL unit
                                        break;
                                    } else if stream_data.packet[pos + 1] == 0xff && pos > nal_start + 3 {
                                        // check for 0xff padding and that we are at least 2 bytes into the nal
                                        break;
                                    } else if stream_data.packet[pos..pos + 3] == [0x00, 0x00, 0x00] && pos > nal_start + 3 {
                                        // check for 0x00 0x00 0x00 0x00 sequence to stop at
                                        break;
                                    }
                                    pos += 1;
                                }

                                // check if we only have 4 bytes left in the packet, if so then collect them too
                                if pos + 4 >= packet_end {
                                    while pos < packet_end {
                                        if stream_data.packet[pos..pos + 1] == [0xff] {
                                            // check for 0xff padding and that we are at least 2 bytes into the nal
                                            break;
                                        } else if pos + 2 < packet_end && stream_data.packet[pos..pos + 2] == [0x00, 0x00] {
                                            // check for 0x00 0x00 sequence to stop at
                                            break;
                                        }
                                        pos += 1;
                                    }
                                }

                                let nal_end = pos; // End of NAL unit found or end of packet
                                if nal_end - nal_start > 3 { // Threshold for significant NAL unit size
                                    let nal_unit = &stream_data.packet[nal_start..nal_end];

                                    // Debug print the NAL unit
                                    if debug_nals {
                                        let packet_len = nal_end - nal_start;
                                        let nal_unit_arc = Arc::new(nal_unit.to_vec());
                                        hexdump(&nal_unit_arc, 0, packet_len);
                                        info!("Extracted {} byte Long NAL Unit from packet range {}-{}:", packet_len, nal_start, nal_end);
                                    }

                                    // Process the NAL unit
                                    annexb_reader.push(nal_unit);
                                    annexb_reader.reset();
                                }
                            } else {
                                pos += 1; // Move to the next byte if no start code found
                            }
                        }
                    }
                    // Clear the batch after processing
                    batch.clear();
                }
                _ = tokio::time::sleep(Duration::from_millis(10)), if !running_decoder.load(Ordering::SeqCst) => {
                    // This branch allows checking the running flag regularly
                    info!("Decoder thread received stop signal.");
                    break;
                }
            }
        }
    });

    /*
    // Process the NAL unit
    annexb_reader.push(nal_unit);
    annexb_reader.reset();
    */
}
