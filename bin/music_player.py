#!/usr/bin/env python3
import os
import subprocess
from hashlib import md5

# Configuration
music_dir = "/Volumes/BrahmaSSD/music/AiGen"
output_file = "/tmp/combined_playlist.wav"
playlist_file = "/tmp/ffmpeg_playlist.txt"
checksum_file = "/tmp/playlist_checksum.txt"

def get_files_sorted_by_mtime(directory, extension=".wav"):
    files = []
    for root, dirs, filenames in os.walk(directory):
        for filename in filenames:
            if filename.endswith(extension):
                full_path = os.path.join(root, filename)
                files.append(full_path)
    return sorted(files, key=os.path.getmtime)

def generate_playlist(files, playlist_path):
    with open(playlist_path, 'w') as playlist:
        for file in files:
            playlist.write(f"file '{file}'\n")

def calculate_checksum(files):
    hash_md5 = md5()
    for file in files:
        with open(file, "rb") as f:
            for chunk in iter(lambda: f.read(4096), b""):
                hash_md5.update(chunk)
    return hash_md5.hexdigest()

def read_previous_checksum(checksum_path):
    try:
        with open(checksum_path, 'r') as file:
            return file.read().strip()
    except FileNotFoundError:
        return ''

def write_new_checksum(checksum, checksum_path):
    with open(checksum_path, 'w') as file:
        file.write(checksum)

def concatenate_files(playlist_path, output_path):
    cmd = ['ffmpeg', '-y', '-hide_banner', '-f', 'concat', '-safe', '0', '-i', playlist_path, '-c', 'copy', output_path]
    print("Running command:", ' '.join(cmd))
    subprocess.run(cmd, check=True)

def play_audio(file_path):
    subprocess.run(['mpv', '--volume=50', file_path], check=True)

while True:
    files = get_files_sorted_by_mtime(music_dir)
    if not files:
        print("No .wav files found in the directory.")
    else:
        current_checksum = calculate_checksum(files)
        previous_checksum = read_previous_checksum(checksum_file)

        if current_checksum != previous_checksum or not os.path.exists(output_file):
            print("Changes detected or output file missing, regenerating...")
            generate_playlist(files, playlist_file)
            concatenate_files(playlist_file, output_file)
            write_new_checksum(current_checksum, checksum_file)
        else:
            print("No changes detected. Using existing combined audio file.")

        print("Playing combined playlist...")
        play_audio(output_file)

