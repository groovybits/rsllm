/*
 * stream_data.rs
 *
 * Data structure for the stream data
*/

use crate::current_unix_timestamp_ms;
use ahash::AHashMap;
use lazy_static::lazy_static;
use log::{debug, error, info};
use rtp::RtpReader;
use rtp_rs as rtp;
use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc, sync::Mutex};

// global variable to store the MpegTS PID Map (initially empty)
lazy_static! {
    static ref PID_MAP: Mutex<AHashMap<u16, Arc<StreamData>>> = Mutex::new(AHashMap::new());
}

// constant for PAT PID
pub const PAT_PID: u16 = 0;
pub const TS_PACKET_SIZE: usize = 188;

pub struct PatEntry {
    pub program_number: u16,
    pub pmt_pid: u16,
}

pub struct PmtEntry {
    pub stream_pid: u16,
    pub stream_type: u8, // Stream type (e.g., 0x02 for MPEG video)
}

pub struct Pmt {
    pub entries: Vec<PmtEntry>,
}

#[derive(Clone, PartialEq)]
pub enum Codec {
    NONE,
    MPEG2,
    H264,
    H265,
}

impl fmt::Display for Codec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Codec::NONE => write!(f, "NONE"),
            Codec::MPEG2 => write!(f, "MPEG2"),
            Codec::H264 => write!(f, "H264"),
            Codec::H265 => write!(f, "H265"),
        }
    }
}

// StreamData struct
#[derive(Serialize, Deserialize, Debug)]
pub struct StreamData {
    pub pid: u16,
    pub pmt_pid: u16,
    pub program_number: u16,
    pub stream_type: String, // "video", "audio", "text"
    pub continuity_counter: u8,
    pub timestamp: u64,
    pub bitrate: u32,
    pub bitrate_max: u32,
    pub bitrate_min: u32,
    pub bitrate_avg: u32,
    pub iat: u64,
    pub iat_max: u64,
    pub iat_min: u64,
    pub iat_avg: u64,
    pub error_count: u32,
    pub last_arrival_time: u64,
    pub start_time: u64, // field for start time
    pub total_bits: u64, // field for total bits
    pub count: u32,      // field for count
    #[serde(skip)]
    pub packet: Arc<Vec<u8>>, // The actual MPEG-TS packet data
    pub packet_start: usize, // Offset into the data
    pub packet_len: usize, // Offset into the data
    // SMPTE 2110 fields
    pub rtp_timestamp: u32,
    pub rtp_payload_type: u8,
    pub rtp_payload_type_name: String,
    pub rtp_line_number: u16,
    pub rtp_line_offset: u16,
    pub rtp_line_length: u16,
    pub rtp_field_id: u8,
    pub rtp_line_continuation: u8,
    pub rtp_extended_sequence_number: u16,
}

impl Clone for StreamData {
    fn clone(&self) -> Self {
        StreamData {
            pid: self.pid,
            pmt_pid: self.pmt_pid,
            program_number: self.program_number,
            stream_type: self.stream_type.clone(),
            continuity_counter: self.continuity_counter,
            timestamp: self.timestamp,
            bitrate: self.bitrate,
            bitrate_max: self.bitrate_max,
            bitrate_min: self.bitrate_min,
            bitrate_avg: self.bitrate_avg,
            iat: self.iat,
            iat_max: self.iat_max,
            iat_min: self.iat_min,
            iat_avg: self.iat_avg,
            error_count: self.error_count,
            last_arrival_time: self.last_arrival_time,
            start_time: self.start_time,
            total_bits: self.total_bits,
            count: self.count,
            packet: Arc::new(Vec::new()), // Initialize as empty with Arc
            packet_start: 0,
            packet_len: 0,
            rtp_timestamp: self.rtp_timestamp,
            rtp_payload_type: self.rtp_payload_type,
            rtp_payload_type_name: self.rtp_payload_type_name.clone(),
            rtp_line_number: self.rtp_line_number,
            rtp_line_offset: self.rtp_line_offset,
            rtp_line_length: self.rtp_line_length,
            rtp_field_id: self.rtp_field_id,
            rtp_line_continuation: self.rtp_line_continuation,
            rtp_extended_sequence_number: self.rtp_extended_sequence_number,
        }
    }
}

// StreamData implementation
impl StreamData {
    pub fn new(
        packet: Arc<Vec<u8>>,
        packet_start: usize,
        packet_len: usize,
        pid: u16,
        stream_type: String,
        start_time: u64,
        timestamp: u64,
        continuity_counter: u8,
    ) -> Self {
        let last_arrival_time = current_unix_timestamp_ms().unwrap_or(0);
        StreamData {
            pid,
            pmt_pid: 0xFFFF,
            program_number: 0,
            stream_type,
            continuity_counter,
            timestamp,
            bitrate: 0,
            bitrate_max: 0,
            bitrate_min: 0,
            bitrate_avg: 0,
            iat: 0,
            iat_max: 0,
            iat_min: 0,
            iat_avg: 0,
            error_count: 0,
            last_arrival_time,
            start_time,    // Initialize start time
            total_bits: 0, // Initialize total bits
            count: 0,      // Initialize count
            packet: packet,
            packet_start: packet_start,
            packet_len: packet_len,
            // SMPTE 2110 fields
            rtp_timestamp: 0,
            rtp_payload_type: 0,
            rtp_payload_type_name: "".to_string(),
            rtp_line_number: 0,
            rtp_line_offset: 0,
            rtp_line_length: 0,
            rtp_field_id: 0,
            rtp_line_continuation: 0,
            rtp_extended_sequence_number: 0,
        }
    }
    // set RTP fields
    pub fn set_rtp_fields(
        &mut self,
        rtp_timestamp: u32,
        rtp_payload_type: u8,
        rtp_payload_type_name: String,
        rtp_line_number: u16,
        rtp_line_offset: u16,
        rtp_line_length: u16,
        rtp_field_id: u8,
        rtp_line_continuation: u8,
        rtp_extended_sequence_number: u16,
    ) {
        self.rtp_timestamp = rtp_timestamp;
        self.rtp_payload_type = rtp_payload_type;
        self.rtp_payload_type_name = rtp_payload_type_name;
        self.rtp_line_number = rtp_line_number;
        self.rtp_line_offset = rtp_line_offset;
        self.rtp_line_length = rtp_line_length;
        self.rtp_field_id = rtp_field_id;
        self.rtp_line_continuation = rtp_line_continuation;
        self.rtp_extended_sequence_number = rtp_extended_sequence_number;
    }
    pub fn update_stream_type(&mut self, stream_type: String) {
        self.stream_type = stream_type;
    }
    pub fn increment_error_count(&mut self, error_count: u32) {
        self.error_count += error_count;
    }
    pub fn increment_count(&mut self, count: u32) {
        self.count += count;
    }
    pub fn set_continuity_counter(&mut self, continuity_counter: u8) {
        // check for continuity continuous increment and wrap around from 0 to 15
        let previous_continuity_counter = self.continuity_counter;
        self.continuity_counter = continuity_counter & 0x0F;
        // check if we incremented without loss
        if self.continuity_counter != previous_continuity_counter + 1
            && self.continuity_counter != previous_continuity_counter
        {
            // check if we wrapped around from 15 to 0
            if self.continuity_counter == 0 {
                // check if previous value was 15
                if previous_continuity_counter == 15 {
                    // no loss
                    return;
                }
            }
            // loss
            self.increment_error_count(1);
            error!(
                "Continuity Counter Error: PID: {} Previous: {} Current: {}",
                self.pid, previous_continuity_counter, self.continuity_counter
            );
        }
        self.continuity_counter = continuity_counter;
    }
    pub fn update_stats(&mut self, packet_size: usize, arrival_time: u64) {
        let bits = packet_size as u64 * 8; // Convert bytes to bits

        // Elapsed time in milliseconds
        let elapsed_time_ms = arrival_time.checked_sub(self.start_time).unwrap_or(0);

        if elapsed_time_ms > 0 {
            let elapsed_time_sec = elapsed_time_ms as f64 / 1000.0;
            self.bitrate = (self.total_bits as f64 / elapsed_time_sec) as u32;

            // Bitrate max
            if self.bitrate > self.bitrate_max {
                self.bitrate_max = self.bitrate;
            }

            // Bitrate min
            if self.bitrate < self.bitrate_min {
                self.bitrate_min = self.bitrate;
            }

            // Bitrate avg
            self.bitrate_avg = (self.bitrate_avg + self.bitrate) / 2;
        }

        self.total_bits += bits; // Accumulate total bits

        // IAT calculation remains the same
        let iat = arrival_time
            .checked_sub(self.last_arrival_time)
            .unwrap_or(0);
        self.iat = iat;

        // IAT max
        if iat > self.iat_max {
            self.iat_max = iat;
        }

        // IAT min
        if iat < self.iat_min {
            self.iat_min = iat;
        }

        // IAT avg
        self.iat_avg = (self.iat_avg + iat) / 2;

        self.last_arrival_time = arrival_time;
    }
}

pub struct Tr101290Errors {
    // p1 errors
    pub ts_sync_byte_errors: u32,
    pub sync_byte_errors: u32,
    pub continuity_counter_errors: u32,
    pub pat_errors: u32,
    pub pmt_errors: u32,
    pub pid_map_errors: u32,
    // p2 errors
    pub transport_error_indicator_errors: u32,
    pub crc_errors: u32,
    pub pcr_repetition_errors: u32,
    pub pcr_discontinuity_indicator_errors: u32,
    pub pcr_accuracy_errors: u32,
    pub pts_errors: u32,
    pub cat_errors: u32,
}

impl fmt::Display for Tr101290Errors {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "\
            TS Sync Byte Errors: {}, \
            Sync Byte Errors: {}, \
            Continuity Counter Errors: {}, \
            PAT Errors: {}, \
            PMT Errors: {}, \
            PID Map Errors: {}, \
            Transport Error Indicator Errors: {}, \
            CRC Errors: {}, \
            PCR Repetition Errors: {}, \
            PCR Discontinuity Indicator Errors: {}, \
            PCR Accuracy Errors: {}, \
            PTS Errors: {}, \
            CAT Errors: {}",
            self.ts_sync_byte_errors,
            self.sync_byte_errors,
            self.continuity_counter_errors,
            self.pat_errors,
            self.pmt_errors,
            self.pid_map_errors,
            // p2 errors
            self.transport_error_indicator_errors,
            self.crc_errors,
            self.pcr_repetition_errors,
            self.pcr_discontinuity_indicator_errors,
            self.pcr_accuracy_errors,
            self.pts_errors,
            self.cat_errors
        )
    }
}

impl Tr101290Errors {
    pub fn new() -> Self {
        Tr101290Errors {
            ts_sync_byte_errors: 0,
            sync_byte_errors: 0,
            continuity_counter_errors: 0,
            pat_errors: 0,
            pmt_errors: 0,
            pid_map_errors: 0,
            // p2
            transport_error_indicator_errors: 0,
            crc_errors: 0,
            pcr_repetition_errors: 0,
            pcr_discontinuity_indicator_errors: 0,
            pcr_accuracy_errors: 0,
            pts_errors: 0,
            cat_errors: 0,
        }
    }
}

// TR 101 290 Priority 1 Check
pub fn tr101290_p1_check(packet: &[u8], errors: &mut Tr101290Errors) {
    // p1
    if packet[0] != 0x47 {
        errors.sync_byte_errors += 1;
    }

    // TODO: ... other checks, updating the respective counters ...
}

// TR 101 290 Priority 2 Check
pub fn tr101290_p2_check(packet: &[u8], errors: &mut Tr101290Errors) {
    // p2

    if (packet[1] & 0x80) != 0 {
        errors.transport_error_indicator_errors += 1;
    }
    // TODO: ... other checks, updating the respective counters ...
}

// Implement a function to extract PID from a packet
pub fn extract_pid(packet: &[u8]) -> u16 {
    if packet.len() < TS_PACKET_SIZE {
        return 0; // Packet size is incorrect
    }

    let transport_error = (packet[1] & 0x80) != 0;
    if transport_error {
        return 0xFFFF; // Packet has a transport error
    }

    // Extract PID from packet
    ((packet[1] as u16 & 0x1F) << 8) | packet[2] as u16
}

// Story the PAT packet and PMT PID
pub struct PmtInfo {
    pub pid: u16,
    pub packet: Vec<u8>,
}

// Helper function to parse PAT and update global PAT packet storage
pub fn parse_and_store_pat(packet: &[u8]) -> PmtInfo {
    let pat_entries = parse_pat(packet);
    let mut pmt_info = PmtInfo {
        pid: 0xFFFF,
        packet: Vec::new(),
    };
    pmt_info.packet = packet.to_vec();

    // Assuming there's only one program for simplicity, update PMT PID
    if let Some(first_entry) = pat_entries.first() {
        pmt_info.pid = first_entry.pmt_pid;
    }
    pmt_info
}

pub fn parse_pat(packet: &[u8]) -> Vec<PatEntry> {
    let mut entries = Vec::new();

    // Check for minimum packet size
    if packet.len() < TS_PACKET_SIZE {
        return entries;
    }

    // Check if Payload Unit Start Indicator (PUSI) is set
    let pusi = (packet[1] & 0x40) != 0;
    if !pusi {
        // If Payload Unit Start Indicator is not set, this packet does not start a new PAT
        return entries;
    }

    let adaptation_field_control = (packet[3] & 0x30) >> 4;
    let mut offset = 4; // start after TS header

    // Check for adaptation field and skip it
    if adaptation_field_control == 0x02 || adaptation_field_control == 0x03 {
        let adaptation_field_length = packet[4] as usize;
        offset += 1 + adaptation_field_length; // +1 for the length byte itself
    }

    // Pointer field indicates the start of the PAT section
    let pointer_field = packet[offset] as usize;
    offset += 1 + pointer_field; // Skip pointer field

    // Now, 'offset' points to the start of the PAT section
    while offset + 4 <= packet.len() {
        let program_number = ((packet[offset] as u16) << 8) | (packet[offset + 1] as u16);
        let pmt_pid = (((packet[offset + 2] as u16) & 0x1F) << 8) | (packet[offset + 3] as u16);

        // Only add valid entries (non-zero program_number and pmt_pid)
        if program_number != 0 && pmt_pid != 0 && pmt_pid < 0x1FFF && program_number < 100 {
            entries.push(PatEntry {
                program_number,
                pmt_pid,
            });
        }

        debug!(
            "ParsePAT: Program Number: {} PMT PID: {}",
            program_number, pmt_pid
        );

        offset += 4; // Move to the next PAT entry
    }

    entries
}

pub fn parse_pmt(packet: &[u8]) -> Pmt {
    let mut entries = Vec::new();
    let program_number = ((packet[8] as u16) << 8) | (packet[9] as u16);

    // Calculate the starting position for stream entries
    let section_length = (((packet[6] as usize) & 0x0F) << 8) | packet[7] as usize;
    let program_info_length = (((packet[15] as usize) & 0x0F) << 8) | packet[16] as usize;
    let mut i = 17 + program_info_length; // Starting index of the first stream in the PMT

    debug!(
        "ParsePMT: Program Number: {} PMT PID: {} starting at position {}",
        program_number,
        extract_pid(packet),
        i
    );
    while i + 5 <= packet.len() && i < 17 + section_length - 4 {
        let stream_type = packet[i];
        let stream_pid = (((packet[i + 1] as u16) & 0x1F) << 8) | (packet[i + 2] as u16);
        let es_info_length = (((packet[i + 3] as usize) & 0x0F) << 8) | packet[i + 4] as usize;
        i += 5 + es_info_length; // Update index to point to next stream's info

        entries.push(PmtEntry {
            stream_pid,
            stream_type,
        });
        debug!(
            "ParsePMT: Stream PID: {}, Stream Type: {}",
            stream_pid, stream_type
        );
    }

    Pmt { entries }
}

// Invoke this function for each MPEG-TS packet
pub fn process_packet(
    stream_data_packet: &mut StreamData,
    errors: &mut Tr101290Errors,
    is_mpegts: bool,
    pmt_pid: u16,
) {
    let packet: &[u8] = &stream_data_packet.packet[stream_data_packet.packet_start
        ..stream_data_packet.packet_start + stream_data_packet.packet_len];
    tr101290_p1_check(packet, errors);
    tr101290_p2_check(packet, errors);

    let pid = stream_data_packet.pid;
    let arrival_time = current_unix_timestamp_ms().unwrap_or(0);

    let mut pid_map = PID_MAP.lock().unwrap();

    // TODO: high debug level output, may need a flag specific to this dump
    //info!("PID Map Contents: {:#?}", pid_map);

    // Check if the PID map already has an entry for this PID
    match pid_map.get_mut(&pid) {
        Some(stream_data_arc) => {
            // Existing StreamData instance found, update it
            let mut stream_data = Arc::clone(stream_data_arc);
            Arc::make_mut(&mut stream_data).update_stats(packet.len(), arrival_time);
            Arc::make_mut(&mut stream_data).increment_count(1);
            if stream_data.pid != 0x1FFF && is_mpegts {
                Arc::make_mut(&mut stream_data)
                    .set_continuity_counter(stream_data_packet.continuity_counter);
            }
            let uptime = arrival_time - stream_data.start_time;

            // print out each field of structure
            debug!("STATUS::PACKET:MODIFY[{}] pid: {} stream_type: {} bitrate: {} bitrate_max: {} bitrate_min: {} bitrate_avg: {} iat: {} iat_max: {} iat_min: {} iat_avg: {} errors: {} continuity_counter: {} timestamp: {} uptime: {} packet_offset: {}, packet_len: {}",
                stream_data.pid, stream_data.pid, stream_data.stream_type, stream_data.bitrate, stream_data.bitrate_max, stream_data.bitrate_min, stream_data.bitrate_avg, stream_data.iat, stream_data.iat_max, stream_data.iat_min, stream_data.iat_avg, stream_data.error_count, stream_data.continuity_counter, stream_data.timestamp, uptime, stream_data_packet.packet_start, stream_data_packet.packet_len);

            stream_data_packet.bitrate = stream_data.bitrate;
            stream_data_packet.bitrate_avg = stream_data.bitrate_avg;
            stream_data_packet.bitrate_max = stream_data.bitrate_max;
            stream_data_packet.bitrate_min = stream_data.bitrate_min;
            stream_data_packet.iat = stream_data.iat;
            stream_data_packet.iat_avg = stream_data.iat_avg;
            stream_data_packet.iat_max = stream_data.iat_max;
            stream_data_packet.iat_min = stream_data.iat_min;
            stream_data_packet.stream_type = stream_data.stream_type.clone();
            stream_data_packet.start_time = stream_data.start_time;
            stream_data_packet.error_count = stream_data.error_count;
            stream_data_packet.last_arrival_time = stream_data.last_arrival_time;
            stream_data_packet.total_bits = stream_data.total_bits;
            stream_data_packet.count = stream_data.count;

            // write the stream_data back to the pid_map with modified values
            pid_map.insert(pid, stream_data);
        }
        None => {
            // No StreamData instance found for this PID, possibly no PMT yet
            if pmt_pid != 0xFFFF {
                debug!("ProcessPacket: New PID {} Found, adding to PID map.", pid);
            } else {
                // PMT packet not found yet, add the stream_data_packet to the pid_map
                let mut stream_data = Arc::new(StreamData::new(
                    Arc::new(Vec::new()), // Ensure packet_data is Arc<Vec<u8>>
                    0,
                    0,
                    stream_data_packet.pid,
                    stream_data_packet.stream_type.clone(),
                    stream_data_packet.start_time,
                    stream_data_packet.timestamp,
                    stream_data_packet.continuity_counter,
                ));
                Arc::make_mut(&mut stream_data).update_stats(packet.len(), arrival_time);

                // print out each field of structure
                info!("STATUS::PACKET:ADD[{}] pid: {} stream_type: {} bitrate: {} bitrate_max: {} bitrate_min: {} bitrate_avg: {} iat: {} iat_max: {} iat_min: {} iat_avg: {} errors: {} continuity_counter: {} timestamp: {} uptime: {}", stream_data.pid, stream_data.pid, stream_data.stream_type, stream_data.bitrate, stream_data.bitrate_max, stream_data.bitrate_min, stream_data.bitrate_avg, stream_data.iat, stream_data.iat_max, stream_data.iat_min, stream_data.iat_avg, stream_data.error_count, stream_data.continuity_counter, stream_data.timestamp, 0);

                pid_map.insert(pid, stream_data);
            }
        }
    }
}

// Use the stored PAT packet
pub fn update_pid_map(pmt_packet: &[u8], last_pat_packet: &[u8]) {
    let mut pid_map = PID_MAP.lock().unwrap();

    // Process the stored PAT packet to find program numbers and corresponding PMT PIDs
    let program_pids = last_pat_packet
        .chunks_exact(TS_PACKET_SIZE)
        .flat_map(parse_pat)
        .collect::<Vec<_>>();

    for pat_entry in program_pids.iter() {
        let program_number = pat_entry.program_number;
        let pmt_pid = pat_entry.pmt_pid;

        // Log for debugging
        debug!(
            "UpdatePIDmap: Processing Program Number: {}, PMT PID: {}",
            program_number, pmt_pid
        );

        // Ensure the current PMT packet matches the PMT PID from the PAT
        if extract_pid(pmt_packet) == pmt_pid {
            let pmt = parse_pmt(pmt_packet);

            for pmt_entry in pmt.entries.iter() {
                debug!(
                    "UpdatePIDmap: Processing PMT PID: {} for Stream PID: {} Type {}",
                    pmt_pid, pmt_entry.stream_pid, pmt_entry.stream_type
                );

                let stream_pid = pmt_entry.stream_pid;
                let stream_type = match pmt_entry.stream_type {
                    0x00 => "Reserved",
                    0x01 => "ISO/IEC 11172 MPEG-1 Video",
                    0x02 => "ISO/IEC 13818-2 MPEG-2 Video",
                    0x03 => "ISO/IEC 11172 MPEG-1 Audio",
                    0x04 => "ISO/IEC 13818-3 MPEG-2 Audio",
                    0x05 => "ISO/IEC 13818-1 Private Section",
                    0x06 => "ISO/IEC 13818-1 Private PES data packets",
                    0x07 => "ISO/IEC 13522 MHEG",
                    0x08 => "ISO/IEC 13818-1 Annex A DSM CC",
                    0x09 => "H222.1",
                    0x0A => "ISO/IEC 13818-6 type A",
                    0x0B => "ISO/IEC 13818-6 type B",
                    0x0C => "ISO/IEC 13818-6 type C",
                    0x0D => "ISO/IEC 13818-6 type D",
                    0x0E => "ISO/IEC 13818-1 auxillary",
                    0x0F => "13818-7 AAC Audio with ADTS transport syntax",
                    0x10 => "14496-2 Visual (MPEG-4 part 2 video)",
                    0x11 => "14496-3 MPEG-4 Audio with LATM transport syntax (14496-3/AMD 1)",
                    0x12 => "14496-1 SL-packetized or FlexMux stream in PES packets",
                    0x13 => "14496-1 SL-packetized or FlexMux stream in 14496 sections",
                    0x14 => "ISO/IEC 13818-6 Synchronized Download Protocol",
                    0x15 => "Metadata in PES packets",
                    0x16 => "Metadata in metadata_sections",
                    0x17 => "Metadata in 13818-6 Data Carousel",
                    0x18 => "Metadata in 13818-6 Object Carousel",
                    0x19 => "Metadata in 13818-6 Synchronized Download Protocol",
                    0x1A => "13818-11 MPEG-2 IPMP stream",
                    0x1B => "H.264/14496-10 video (MPEG-4/AVC)",
                    0x24 => "H.265 video (MPEG-H/HEVC)",
                    0x42 => "AVS Video",
                    0x7F => "IPMP stream",
                    0x81 => "ATSC A/52 AC-3",
                    0x86 => "SCTE 35 Splice Information Table",
                    0x87 => "ATSC A/52e AC-3",
                    _ if pmt_entry.stream_type < 0x80 => "ISO/IEC 13818-1 reserved",
                    _ => "User Private",
                };

                let timestamp = current_unix_timestamp_ms().unwrap_or(0);

                if !pid_map.contains_key(&stream_pid) {
                    let mut stream_data = Arc::new(StreamData::new(
                        Arc::new(Vec::new()), // Ensure packet_data is Arc<Vec<u8>>
                        0,
                        0,
                        stream_pid,
                        stream_type.to_string(),
                        timestamp,
                        timestamp,
                        0,
                    ));
                    // update stream_data stats
                    Arc::make_mut(&mut stream_data).update_stats(pmt_packet.len(), timestamp);

                    // print out each field of structure
                    info!("STATUS::STREAM:CREATE[{}] pid: {} stream_type: {} bitrate: {} bitrate_max: {} bitrate_min: {} bitrate_avg: {} iat: {} iat_max: {} iat_min: {} iat_avg: {} errors: {} continuity_counter: {} timestamp: {} uptime: {}", stream_data.pid, stream_data.pid, stream_data.stream_type, stream_data.bitrate, stream_data.bitrate_max, stream_data.bitrate_min, stream_data.bitrate_avg, stream_data.iat, stream_data.iat_max, stream_data.iat_min, stream_data.iat_avg, stream_data.error_count, stream_data.continuity_counter, stream_data.timestamp, 0);

                    pid_map.insert(stream_pid, stream_data);
                } else {
                    // get the stream data so we can update it
                    let stream_data_arc = pid_map.get_mut(&stream_pid).unwrap();
                    let mut stream_data = Arc::clone(stream_data_arc);

                    // update the stream type
                    Arc::make_mut(&mut stream_data).update_stream_type(stream_type.to_string());

                    // print out each field of structure
                    debug!("STATUS::STREAM:UPDATE[{}] pid: {} stream_type: {} bitrate: {} bitrate_max: {} bitrate_min: {} bitrate_avg: {} iat: {} iat_max: {} iat_min: {} iat_avg: {} errors: {} continuity_counter: {} timestamp: {} uptime: {}", stream_data.pid, stream_data.pid, stream_data.stream_type, stream_data.bitrate, stream_data.bitrate_max, stream_data.bitrate_min, stream_data.bitrate_avg, stream_data.iat, stream_data.iat_max, stream_data.iat_min, stream_data.iat_avg, stream_data.error_count, stream_data.continuity_counter, stream_data.timestamp, 0);

                    // write the stream_data back to the pid_map with modified values
                    pid_map.insert(stream_pid, stream_data);
                }
            }
        } else {
            error!("UpdatePIDmap: Skipping PMT PID: {} as it does not match with current PMT packet PID", pmt_pid);
        }
    }
}

pub fn determine_stream_type(pid: u16) -> String {
    let pid_map = PID_MAP.lock().unwrap();

    // check if pid already is mapped, if so return the stream type already stored
    if let Some(stream_data) = pid_map.get(&pid) {
        return stream_data.stream_type.clone();
    }

    pid_map
        .get(&pid)
        .map(|stream_data| stream_data.stream_type.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

// Helper function to identify the video PID from the stored PAT packet and return the PID and codec
pub fn identify_video_pid(pmt_packet: &[u8]) -> Option<(u16, Codec)> {
    let pmt = parse_pmt(pmt_packet);
    pmt.entries.iter().find_map(|entry| {
        let codec = match entry.stream_type {
            0x01..=0x02 => Some(Codec::MPEG2), // MPEG-2 Video
            0x1B => Some(Codec::H264),         // H.264 Video
            0x24 => Some(Codec::H265),         // H.265 Video
            _ => None,
        };
        codec.map(|c| (entry.stream_pid, c))
    })
}

// Check if the packet is MPEG-TS or SMPTE 2110
pub fn is_mpegts_or_smpte2110(packet: &[u8]) -> i32 {
    // Check for MPEG-TS (starts with 0x47 sync byte)
    if packet.starts_with(&[0x47]) {
        return 1;
    }

    // Basic check for RTP (which SMPTE ST 2110 uses)
    // This checks if the first byte is 0x80 or 0x81
    // This might need more robust checks based on requirements
    if packet.len() > 12 && (packet[0] == 0x80 || packet[0] == 0x81) {
        // TODO: Check payload type or other RTP header fields here if necessary
        return 2; // Assuming it's SMPTE ST 2110 for now
    }

    0 // Not MPEG-TS or SMPTE 2110
}

// ## RFC 4175 SMPTE2110 header functions ##
/*const RFC_4175_EXT_SEQ_NUM_LEN: usize = 2;
const RFC_4175_HEADER_LEN: usize = 6; // Note: extended sequence number not included*/ // TODO: implement RFC 4175 SMPTE2110 header functions

fn get_extended_sequence_number(buf: &[u8]) -> u16 {
    ((buf[0] as u16) << 8) | buf[1] as u16
}

fn get_line_length(buf: &[u8]) -> u16 {
    ((buf[0] as u16) << 8) | buf[1] as u16
}

fn get_line_field_id(buf: &[u8]) -> u8 {
    buf[2] >> 7
}

fn get_line_number(buf: &[u8]) -> u16 {
    ((buf[2] as u16 & 0x7f) << 8) | buf[3] as u16
}

fn get_line_continuation(buf: &[u8]) -> u8 {
    buf[4] >> 7
}

fn get_line_offset(buf: &[u8]) -> u16 {
    ((buf[4] as u16 & 0x7f) << 8) | buf[5] as u16
}
// ## End of RFC 4175 SMPTE2110 header functions ##

// Process the packet and return a vector of SMPTE ST 2110 packets
pub fn process_smpte2110_packet(
    payload_offset: usize,
    packet: Arc<Vec<u8>>,
    _packet_size: usize,
    start_time: u64,
    debug: bool,
) -> Vec<StreamData> {
    let mut streams = Vec::new();
    let mut offset = payload_offset;

    let len = packet.len();

    // Check if the packet is large enough to contain an RTP header
    while offset + 12 <= len {
        // Check for RTP header marker
        let packet_arc = Arc::clone(&packet);
        if packet_arc[offset] == 0x80 || packet_arc[offset] == 0x81 {
            let rtp_packet = &packet[offset..];

            // Create an RtpReader
            if let Ok(rtp) = RtpReader::new(rtp_packet) {
                // Extract the timestamp and payload type
                let timestamp = rtp.timestamp();
                let payload_type = rtp.payload_type();
                let rtp_payload = rtp.payload();
                let rtp_payload_offset = rtp.payload_offset();

                // Extract SMPTE 2110 specific fields
                let line_length = get_line_length(rtp_packet);
                let rtp_packet_size = line_length as usize;
                let line_number = get_line_number(rtp_packet);
                let extended_sequence_number = get_extended_sequence_number(rtp_packet);
                let line_offset = get_line_offset(rtp_packet);
                let field_id = get_line_field_id(rtp_packet);
                let line_continuation = get_line_continuation(rtp_packet);

                // Calculate the length of the RTP payload
                let rtp_payload_length = rtp_payload.len();

                // Use payload type as PID (for the purpose of this example)
                let pid = payload_type as u16;
                let stream_type = payload_type.to_string();

                // Create new StreamData instance
                let mut stream_data = StreamData::new(
                    packet_arc,
                    rtp_payload_offset,
                    rtp_payload_length,
                    pid,
                    stream_type,
                    start_time,
                    timestamp as u64,
                    0,
                );

                // Update StreamData stats and RTP fields
                stream_data
                    .update_stats(rtp_payload_length, current_unix_timestamp_ms().unwrap_or(0));
                stream_data.set_rtp_fields(
                    timestamp,
                    payload_type,
                    payload_type.to_string(),
                    line_number,
                    line_offset,
                    line_length,
                    field_id,
                    line_continuation,
                    extended_sequence_number,
                );
                if debug {
                    info!(
                    "SMPTE ST 2110 packet: offset: {} size: {} timestamp: {}, payload_type: {}, line_number: {}, line_offset: {}, line_length: {}, field_id: {}, line_continuation: {}, extended_sequence_number: {}",
                    rtp_payload_offset, rtp_payload_length, timestamp, payload_type, line_number, line_offset, line_length, field_id, line_continuation, extended_sequence_number
                );
                }

                // Add the StreamData to the stream list
                streams.push(stream_data);

                // Move to the next RTP packet
                offset += rtp_packet_size;
            } else {
                error!("Error parsing RTP header, not SMPTE ST 2110");
            }
        } else {
            error!("No RTP header detected, not SMPTE ST 2110");
        }
    }

    streams
}

// Process the packet and return a vector of MPEG-TS packets
pub fn process_mpegts_packet(
    payload_offset: usize,
    packet: Arc<Vec<u8>>,
    packet_size: usize,
    start_time: u64,
) -> Vec<StreamData> {
    let mut start = payload_offset;
    let mut read_size = packet_size;
    let mut streams = Vec::new();

    let len = packet.len();

    while start + read_size <= len {
        let chunk = &packet[start..start + read_size];
        if chunk[0] == 0x47 {
            // Check for MPEG-TS sync byte
            read_size = packet_size; // reset read_size

            let pid = extract_pid(chunk);

            let stream_type = determine_stream_type(pid); // Implement this function based on PAT/PMT parsing
            let timestamp = ((chunk[4] as u64) << 25)
                | ((chunk[5] as u64) << 17)
                | ((chunk[6] as u64) << 9)
                | ((chunk[7] as u64) << 1)
                | ((chunk[8] as u64) >> 7);
            let continuity_counter = chunk[3] & 0x0F;

            let mut stream_data = StreamData::new(
                Arc::clone(&packet),
                start,
                packet_size,
                pid,
                stream_type,
                start_time,
                timestamp,
                continuity_counter,
            );
            stream_data.update_stats(packet_size, current_unix_timestamp_ms().unwrap_or(0));
            streams.push(stream_data);
        } else {
            error!("ProcessPacket: Not MPEG-TS");
            read_size = 1; // Skip to the next byte
        }
        start += read_size;
    }

    streams
}
