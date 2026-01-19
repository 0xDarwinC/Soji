import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { AppSettings } from "../types";

export function useSettings() {
    const [showSettings, setShowSettings] = useState(false);
    const [settings, setSettings] = useState<AppSettings>({
        sticker_path: "",
        recents_limit: 18,
        theme: "acrylic",
        disable_animations: false,
        max_items: 200,
        run_on_startup: true
    });

    useEffect(() => {
        const init = async () => {
            const s = await invoke<AppSettings>("get_settings");
            setSettings(s);
            
            const currentlyEnabled = await isEnabled();
            if (s.run_on_startup && !currentlyEnabled) {
                await enable();
            } else if (!s.run_on_startup && currentlyEnabled) {
                await disable();
            }
        };
        init();
    }, []);

    const saveSettings = async (newSettings: AppSettings) => {
        setSettings(newSettings);
        await invoke("save_settings", { settings: newSettings });
        try {
            const currentlyEnabled = await isEnabled();
            if (newSettings.run_on_startup && !currentlyEnabled) {
                await enable();
            } else if (!newSettings.run_on_startup && currentlyEnabled) {
                await disable();
            }
        } catch (e) {
            console.error("Failed to toggle autostart:", e);
        }
    };

    const handleThemeChange = (theme: string) => {
        saveSettings({ ...settings, theme });
    };

    const toggleSettings = () => setShowSettings(!showSettings);

    return {
        settings,
        showSettings,
        setShowSettings,
        saveSettings,
        handleThemeChange,
        toggleSettings
    };
}