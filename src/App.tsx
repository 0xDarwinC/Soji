import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

import { useSettings } from "./hooks/useSettings";
import { useLibrary } from "./hooks/useLibrary";
import { SearchBar } from "./components/SearchBar";
import { Header } from "./components/Header/Header";
import { LoadingOverlay } from "./components/LoadingOverlay";
import { SettingsModal } from "./components/Settings/SettingsModal";
import { StickerGrid } from "./components/StickerGrid/StickerGrid";

function App() {
    const { settings, showSettings, setShowSettings, saveSettings, toggleSettings } = useSettings();
    const { 
        stickers, query, activeTab, packs, indexingProgress, 
        handleSearch, handleTabClick, reloadCurrentView, refreshLibrary 
    } = useLibrary();

    const tabsRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);

    // Initial focus
    useEffect(() => {
        setTimeout(() => {
            inputRef.current?.focus();
            inputRef.current?.select();
        }, 50);
    }, []);

    const scrollTags = (direction: 'left' | 'right') => {
        if (tabsRef.current) {
            const scrollAmount = 200;
            tabsRef.current.scrollBy({
                left: direction === 'left' ? -scrollAmount : scrollAmount,
                behavior: 'smooth'
            });
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === "Enter" && stickers.length > 0) {
            // We invoke directly here as it's a specific UI interaction
            invoke("select_sticker", { path: stickers[0].path }).catch(console.error);
        }
        if (e.key === "Escape") {
            invoke("hide_window").catch(() => { });
        }
    };

    const handleClose = () => invoke("hide_window");

    return (
        <div 
            className={settings.disable_animations ? "no-animations" : ""}
            onContextMenu={(e) => e.preventDefault()}
            style={{
                height: "100%", width: "100%", overflow: "hidden",
                boxSizing: "border-box", background: "transparent",
                display: "flex", flexDirection: "column", position: "relative",
                color: "white"
            }}
        >
            <Header onToggleSettings={toggleSettings} onRefresh={reloadCurrentView} onClose={handleClose} />

            {indexingProgress && <LoadingOverlay progress={indexingProgress} />}

            {showSettings && (
                <SettingsModal 
                    settings={settings}
                    onSaveSettings={saveSettings}
                    onClose={() => setShowSettings(false)}
                    onRefreshRequest={refreshLibrary}
                />
            )}

            {!showSettings && (
                <div style={{ flex: 1, display: "flex", flexDirection: "column", padding: "0 20px 20px 20px", overflow: "hidden" }}>
                    {/* SEARCH */}
                    <SearchBar 
                        ref={inputRef}
                        query={query}
                        onSearch={handleSearch}
                        onKeyDown={handleKeyDown}
                    />
                    {/* TAGS NAVIGATION */}
                    {query.length === 0 && (
                        <div style={{ display: "flex", alignItems: "center", gap: "5px", marginBottom: "10px" }}>
                            <button className="tag-nav-btn" onClick={() => scrollTags('left')}>‹</button>

                            <div
                                ref={tabsRef}
                                style={{
                                    flex: 1, display: "flex", gap: "8px", overflowX: "auto",
                                    whiteSpace: "nowrap", paddingBottom: "5px", scrollBehavior: "smooth",
                                    maskImage: "linear-gradient(to right, transparent, black 10px, black 95%, transparent)"
                                }}
                            >
                                {["Recents", "Favorites", "All", ...packs].map(pack => (
                                    <button key={pack} onClick={() => handleTabClick(pack)}
                                        style={{
                                            padding: "6px 14px", borderRadius: "20px", border: "none",
                                            fontSize: "13px", cursor: "pointer",
                                            background: activeTab === pack ? "white" : "rgba(255,255,255,0.1)",
                                            color: activeTab === pack ? "black" : "white",
                                            transition: "all 0.2s", fontWeight: activeTab === pack ? "bold" : "normal",
                                            flexShrink: 0
                                        }}
                                    >
                                        {pack}
                                    </button>
                                ))}
                            </div>

                            <button className="tag-nav-btn" onClick={() => scrollTags('right')}>›</button>
                        </div>
                    )}

                    <StickerGrid stickers={stickers} packs={packs} onReload={reloadCurrentView} />
                </div>
            )}
        </div>
    );
}

export default App;