import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppSettings } from "../types";

export function useSettings() {
    const [showSettings, setShowSettings] = useState(false);
    const [settings, setSettings] = useState<AppSettings>({
        sticker_path: "",
        recents_limit: 18,
        theme: "acrylic",
        disable_animations: false
    });

    useEffect(() => {
        invoke<AppSettings>("get_settings").then(setSettings);
    }, []);

    const saveSettings = async (newSettings: AppSettings) => {
        setSettings(newSettings);
        await invoke("save_settings", { settings: newSettings });
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