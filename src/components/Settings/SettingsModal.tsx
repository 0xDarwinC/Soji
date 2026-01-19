import React, {useState} from 'react';
import { invoke } from "@tauri-apps/api/core";
import { open, ask } from '@tauri-apps/plugin-dialog';
import { AppSettings } from '../../types';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import './Settings.css';

interface SettingsModalProps {
    settings: AppSettings;
    onSaveSettings: (s: AppSettings) => void;
    onClose: () => void;
    onRefreshRequest: () => void;
}

export const SettingsModal: React.FC<SettingsModalProps> = ({ settings, onSaveSettings, onClose, onRefreshRequest }) => {
    const [updateStatus, setUpdateStatus] = useState("");
    const handleCheckUpdate = async () => {
        setUpdateStatus("Checking...");
        try {
            const update = await check();
            if (update) {
                setUpdateStatus(`Update found: ${update.version}`);
                const yes = await ask(`Update to ${update.version} is available.\n\n${update.body}`, {
                    title: 'Update Available',
                    kind: 'info',
                    okLabel: 'Update & Restart'
                });
                if (yes) {
                    setUpdateStatus("Downloading...");
                    await update.downloadAndInstall();
                    setUpdateStatus("Restarting...");
                    await relaunch();
                } else {
                    setUpdateStatus("Update skipped.");
                }
            } else {
                setUpdateStatus("You are up to date.");
            }
        } catch (e) {
            console.error(e);
            setUpdateStatus("Error checking updates.");
        }
    };
    const handleChooseFolder = async () => {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                defaultPath: settings.sticker_path || undefined,
            });

            if (selected) {
                const newPath = selected as string;
                const updatedSettings = { ...settings, sticker_path: newPath };
                onSaveSettings(updatedSettings);

                const confirmed = await ask(
                    "You've changed the sticker directory. To see the new images, the library cache must be reset. Do you want to reset it now?",
                    { title: 'Update Library?', kind: 'info' }
                );

                if (confirmed) {
                    await invoke("wipe_data", { dataType: "db" });
                    await invoke("refresh_library");
                    setTimeout(onRefreshRequest, 100);
                }
            }
        } catch (err) {
            console.error(err);
        }
    };

    const handleWipeData = async (type: "history" | "favorites" | "db") => {
        let message = `Are you sure you want to wipe your ${type}?`;
        if (type === "db") {
            message = "This will remove all stickers from the index and delete all cached thumbnails. The app will need to regenerate them, which may take time.";
        }

        const confirmed = await ask(message, { title: 'Confirm Wipe', kind: 'warning' });

        if (confirmed) {
            await invoke("wipe_data", { dataType: type });
            await invoke("refresh_library");
            setTimeout(onRefreshRequest, 100);
        }
    };

    const handleRestoreDefaults = async () => {
        const defaults: AppSettings = {
            sticker_path: settings.sticker_path,
            recents_limit: 18,
            theme: "acrylic",
            disable_animations: false,
            max_items: 200,
            run_on_startup: true
        };
        onSaveSettings(defaults);
    };

    return (
        <div className="settings-overlay" onClick={onClose} onContextMenu={(e) => e.preventDefault()}>
            <div
                className="settings-modal"
                onClick={(e) => e.stopPropagation()}
            >
                <h2 className="settings-header">Settings</h2>

                {/* Library */}
                <div className="settings-section">
                    <label className="settings-label">Library</label>
                    <div className="settings-row">
                        <input 
                            readOnly 
                            value={settings.sticker_path || "Default (Pictures/Stickers)"} 
                            className="settings-input-readonly" 
                        />
                        <button onClick={handleChooseFolder} className="settings-btn-primary">Change</button>
                    </div>
                </div>

                {/* APPEARANCE */}
                <div className="settings-section">
                    <label className="settings-label">Appearance</label>
                    <div className="settings-row">
                        <button 
                            onClick={() => onSaveSettings({...settings, theme: "acrylic"})} 
                            className="settings-btn-toggle"
                            style={{ 
                                background: settings.theme === "acrylic" ? "white" : "transparent", 
                                color: settings.theme === "acrylic" ? "black" : "white" 
                            }}
                        >
                            Acrylic (Blur)
                        </button>
                        <button 
                            onClick={() => onSaveSettings({...settings, theme: "mica"})} 
                            className="settings-btn-toggle"
                            style={{ 
                                background: settings.theme === "mica" ? "white" : "transparent", 
                                color: settings.theme === "mica" ? "black" : "white" 
                            }}
                        >
                            Mica (Tint)
                        </button>
                    </div>
                    <div className="settings-row" style={{ marginTop: "5px" }}>
                        <input 
                            type="checkbox" 
                            id="disableAnimations"
                            checked={settings.disable_animations}
                            onChange={(e) => onSaveSettings({ ...settings, disable_animations: e.target.checked })}
                            style={{ transform: "scale(1.2)", cursor: "pointer" }}
                        />
                        <label htmlFor="disableAnimations" style={{ fontSize: "14px", cursor: "pointer" }}>Disable Animations</label>
                    </div>
                    <div className="settings-row">
                        <input 
                            type="checkbox" 
                            id="runOnStartup"
                            checked={settings.run_on_startup}
                            onChange={(e) => onSaveSettings({ ...settings, run_on_startup: e.target.checked })}
                            style={{ transform: "scale(1.2)", cursor: "pointer" }}
                        />
                        <label htmlFor="runOnStartup" style={{ fontSize: "14px", cursor: "pointer" }}>Run on Startup</label>
                    </div>
                </div>

                {/* Data */}
                <div className="settings-section">
                    <label className="settings-label">Data</label>
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                        <span style={{ fontSize: "14px", color: "white" }}>Recents Limit</span>
                        <input 
                            type="number" 
                            min="1" 
                            max="100" 
                            value={settings.recents_limit} 
                            onChange={(e) => onSaveSettings({ ...settings, recents_limit: parseInt(e.target.value) || 18 })} 
                            style={{ width: "60px", padding: "6px", borderRadius: "5px", border: "none", background: "rgba(0,0,0,0.5)", color: "white", textAlign: "center" }} 
                        />
                    </div>
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: "5px" }}>
                        <span style={{ fontSize: "14px", color: "white" }}>Max Items (-1 for All)</span>
                        <input 
                            type="number" 
                            min="-1" max="10000" 
                            value={settings.max_items} 
                            onChange={(e) => onSaveSettings({ ...settings, max_items: parseInt(e.target.value) || 200 })} 
                            style={{ width: "60px", padding: "6px", borderRadius: "5px", border: "none", background: "rgba(0,0,0,0.5)", color: "white", textAlign: "center" }} 
                        />
                    </div>
                    <div className="settings-row" style={{ marginTop: "10px" }}>
                        <button onClick={() => handleWipeData("history")} className="settings-btn-danger">Wipe History</button>
                        <button onClick={() => handleWipeData("favorites")} className="settings-btn-danger">Wipe Favs</button>
                    </div>
                    <button onClick={() => handleWipeData("db")} className="settings-btn-danger-block">Reset Library & Cache</button>
                    
                    <button onClick={handleRestoreDefaults} className="settings-btn-default">
                        Restore Default Settings
                    </button>
                {/* UPDATES */}
                <div className="settings-section">
                    <label className="settings-label">Updates</label>
                    <button onClick={handleCheckUpdate} className="settings-btn-primary" style={{width: '100%'}}>
                        Check for Updates
                    </button>
                    {updateStatus && (
                        <div style={{ fontSize: '13px', marginTop: '5px', opacity: 0.7, textAlign: 'center' }}>
                            {updateStatus}
                        </div>
                    )}
                </div>
                </div>
            </div>
        </div>
    );
};