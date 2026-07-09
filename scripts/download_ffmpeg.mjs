import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const targetDir = path.join(__dirname, '..', 'src-tauri', 'bin');
const targetFile = path.join(targetDir, 'ffmpeg-x86_64-pc-windows-msvc.exe');

if (!fs.existsSync(targetDir)) {
    fs.mkdirSync(targetDir, { recursive: true });
}

if (!fs.existsSync(targetFile)) {
    console.log("Downloading FFmpeg sidecar...");
    
    const zipUrl = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";
    const zipPath = path.join(targetDir, "ffmpeg.zip");
    
    try {
        console.log("Fetching zip file...");
        execSync(`powershell -Command "Invoke-WebRequest -Uri '${zipUrl}' -OutFile '${zipPath}'"`, { stdio: 'inherit' });
        
        console.log("Extracting ffmpeg.exe...");
        execSync(`powershell -Command "Expand-Archive -Path '${zipPath}' -DestinationPath '${targetDir}' -Force"`, { stdio: 'inherit' });
        
        const extractedExe = path.join(targetDir, "ffmpeg-master-latest-win64-gpl", "bin", "ffmpeg.exe");
        fs.renameSync(extractedExe, targetFile);
        
        console.log("Cleaning up...");
        fs.rmSync(zipPath);
        fs.rmSync(path.join(targetDir, "ffmpeg-master-latest-win64-gpl"), { recursive: true, force: true });
        
        console.log("FFmpeg sidecar downloaded successfully.");
    } catch (e) {
        console.error("Failed to download FFmpeg:", e.message);
        process.exit(1);
    }
} else {
    console.log("FFmpeg sidecar already exists. Skipping download.");
}
