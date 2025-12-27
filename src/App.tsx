import { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, ask } from '@tauri-apps/plugin-dialog';

interface Sticker {
  name: string;
  path: string;
  format: string;
  pack: string;
  is_favorite: boolean,
  rec_score: number,
}

interface AppSettings {
  sticker_path: string;
  recents_limit: number;
  theme: string;
}

function App() {
  const [stickers, setStickers] = useState<Sticker[]>([]);
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState("All");
  const [showSettings, setShowSettings] = useState(false);
  const [settings, setSettings] = useState<AppSettings>({
      sticker_path: "",
      recents_limit: 18,
      theme: "acrylic"
  });
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadStickers("");
    invoke<AppSettings>("get_settings").then(setSettings);

    // listens for the "app_shown" event from Rust
    const unlisten = listen("app_shown", () => {
        loadStickers(""); 
        
        // focus search bar
        setTimeout(() => {
            inputRef.current?.focus();
            inputRef.current?.select();
        }, 50);
    });

    return () => {
        unlisten.then(f => f());
    }
  }, []);

  const loadStickers = async (searchQuery: string) => {
    try {
        const result = await invoke<Sticker[]>("search_stickers", { query: searchQuery });
        setStickers(result);
    } catch (err) {
        console.error(err);
    }
  };

  const saveSettings = async (newSettings: AppSettings) => {
      setSettings(newSettings);
      await invoke("save_settings", { settings: newSettings });
      loadStickers(query);
  };

  const handleChooseFolder = async () => {
    try {
        const selected = await open({
            directory: true,
            multiple: false,
            defaultPath: settings.sticker_path || undefined,
        });

        if (selected) {
            saveSettings({ ...settings, sticker_path: selected as string });
        }
    } catch (err) {
        console.error(err);
    }
  };

  const handleThemeChange = async (theme: string) => {
    const newSettings = { ...settings, theme };
    saveSettings(newSettings); // This calls save_settings which triggers apply_theme in Rust
  };

  const handleWipeData = async (type: "history" | "favorites") => {
      const confirmed = await ask(`Are you sure you want to wipe your ${type}?`, {
          title: 'Confirm Wipe',
          kind: 'warning'
      });

      if (confirmed) {
          await invoke("wipe_data", { dataType: type });
          loadStickers(query);
      }
  };

  const handleSearch = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setQuery(val);
    loadStickers(val);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // enter sends first sticker in list
    if (e.key === "Enter" && stickers.length > 0) {
        handleStickerClick(stickers[0].path);
    }
    // kicks you out
    if (e.key === "Escape") {
        invoke("hide_window").catch(() => {});
    }
  };

  const handleStickerClick = async (path: string) => {
    try {
      await invoke("select_sticker", { path });
    } catch (error) {
      console.error("Failed to select sticker:", error);
    }
  };

  const packs = useMemo(() => {
    const allPacks = Array.from(new Set(stickers.map(s => s.pack))).sort();
    return ["Recents", "Favorites", "All", ...allPacks];
  }, [stickers]);

  const displayedStickers = useMemo(() => {
    if (query.length > 0) return stickers;
    if (activeTab === "Recents") {
        return stickers
            .filter(s => s.rec_score > 0)
            .sort((a, b) => b.rec_score - a.rec_score)
            .slice(0, settings.recents_limit);
    }
    if (activeTab === "Favorites") return stickers.filter(s => s.is_favorite);
    if (activeTab === "All") return stickers;
    return stickers.filter(s => s.pack === activeTab);
  }, [stickers, query, activeTab, settings.recents_limit]);

  const handleToggleFav = async (e: React.MouseEvent, path: string) => {
    e.stopPropagation();
    await invoke("toggle_favorite", { path });
    loadStickers(query);
  };

  return (
    <div style={{ 
      padding: "20px", height: "100%", width: "100%", overflow: "hidden", 
      boxSizing: "border-box", background: "transparent", display: "flex", flexDirection: "column", position: "relative"
    }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "20px" }}>
          <h1 style={{ margin: 0 }}>Soji</h1>
          <button 
            onClick={() => setShowSettings(!showSettings)}
            style={{
                background: "transparent", border: "none", color: "white", fontSize: "20px", cursor: "pointer", opacity: 0.7
            }}
          >
              ⚙
          </button>
      </div>

      {/* SETTINGS MODAL (Fixed Overlay) */}
      {showSettings && (
          <div style={{
              position: "fixed", top: 0, left: 0, width: "100%", height: "100%",
              background: "rgba(0, 0, 0, 0.6)", zIndex: 99,
              display: "flex", alignItems: "center", justifyContent: "center",
              backdropFilter: "blur(5px)"
          }} onClick={() => setShowSettings(false)}>
              
              <div 
                onClick={(e) => e.stopPropagation()} // Prevent click from closing modal
                style={{
                  width: "90%", maxWidth: "400px", maxHeight: "80%",
                  background: "rgba(30, 30, 30, 0.95)", borderRadius: "12px", padding: "25px",
                  display: "flex", flexDirection: "column", gap: "20px", 
                  border: "1px solid rgba(255,255,255,0.1)", boxShadow: "0 10px 30px rgba(0,0,0,0.5)",
                  overflowY: "auto"
              }}>
                  <h2 style={{ margin: "0", borderBottom: "1px solid rgba(255,255,255,0.1)", paddingBottom: "10px" }}>Settings</h2>
                  
                  {/* GENERAL */}
                  <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                      <label style={{ fontSize: "12px", opacity: 0.7, textTransform: "uppercase", letterSpacing: "1px" }}>Library</label>
                      <div style={{ display: "flex", gap: "10px" }}>
                          <input 
                            readOnly 
                            value={settings.sticker_path || "Default (Pictures/Stickers)"} 
                            style={{ flex: 1, padding: "8px", borderRadius: "5px", border: "none", background: "rgba(0,0,0,0.5)", color: "white", fontSize: "12px" }}
                          />
                          <button onClick={handleChooseFolder} style={{ padding: "8px 15px", borderRadius: "5px", border: "none", background: "white", color: "black", cursor: "pointer", fontWeight: "bold" }}>Change</button>
                      </div>
                  </div>

                  {/* APPEARANCE */}
                  <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                      <label style={{ fontSize: "12px", opacity: 0.7, textTransform: "uppercase", letterSpacing: "1px" }}>Appearance</label>
                      <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
                          <button 
                            onClick={() => handleThemeChange("acrylic")}
                            style={{ flex: 1, padding: "8px", borderRadius: "5px", border: "1px solid rgba(255,255,255,0.2)", 
                                     background: settings.theme === "acrylic" ? "white" : "transparent", 
                                     color: settings.theme === "acrylic" ? "black" : "white", cursor: "pointer" }}
                          >
                              Acrylic (Blur)
                          </button>
                          <button 
                            onClick={() => handleThemeChange("mica")}
                            style={{ flex: 1, padding: "8px", borderRadius: "5px", border: "1px solid rgba(255,255,255,0.2)", 
                                     background: settings.theme === "mica" ? "white" : "transparent", 
                                     color: settings.theme === "mica" ? "black" : "white", cursor: "pointer" }}
                          >
                              Mica (Tint)
                          </button>
                      </div>
                  </div>

                  {/* DATA */}
                  <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                      <label style={{ fontSize: "12px", opacity: 0.7, textTransform: "uppercase", letterSpacing: "1px" }}>Data</label>
                      
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                          <span style={{ fontSize: "14px" }}>Recents Limit</span>
                          <input 
                            type="number" min="1" max="100"
                            value={settings.recents_limit}
                            onChange={(e) => saveSettings({...settings, recents_limit: parseInt(e.target.value) || 18})}
                            style={{ width: "60px", padding: "6px", borderRadius: "5px", border: "none", background: "rgba(0,0,0,0.5)", color: "white", textAlign: "center" }}
                          />
                      </div>

                      <div style={{ display: "flex", gap: "10px", marginTop: "5px" }}>
                          <button onClick={() => handleWipeData("history")} style={{ flex: 1, padding: "10px", borderRadius: "5px", border: "1px solid #ff4d4d", background: "rgba(255, 77, 77, 0.1)", color: "#ff4d4d", cursor: "pointer" }}>Wipe History</button>
                          <button onClick={() => handleWipeData("favorites")} style={{ flex: 1, padding: "10px", borderRadius: "5px", border: "1px solid #ff4d4d", background: "rgba(255, 77, 77, 0.1)", color: "#ff4d4d", cursor: "pointer" }}>Wipe Favs</button>
                      </div>
                  </div>

                  <div style={{ marginTop: "auto", display: "flex", justifyContent: "flex-end" }}>
                      <button onClick={() => setShowSettings(false)} style={{ padding: "8px 20px", borderRadius: "5px", border: "1px solid rgba(255,255,255,0.2)", background: "transparent", color: "white", cursor: "pointer" }}>Close</button>
                  </div>
              </div>
          </div>
      )}

      {/* MAIN UI */}
      {!showSettings && (
          <>
            <input 
                ref={inputRef} type="text" placeholder="Search stickers..." value={query} onChange={handleSearch} onKeyDown={handleKeyDown}
                style={{ width: "100%", padding: "12px", marginBottom: "15px", borderRadius: "8px", border: "1px solid rgba(255,255,255,0.2)", background: "rgba(0,0,0,0.3)", color: "white", fontSize: "16px", outline: "none", backdropFilter: "blur(10px)" }}
            />

            {query.length === 0 && (
                <div className="no-scrollbar" style={{ display: "flex", gap: "8px", overflowX: "auto", paddingBottom: "10px", marginBottom: "5px", whiteSpace: "nowrap" }}>              
                    {packs.map(pack => (
                        <button key={pack} onClick={() => setActiveTab(pack)} style={{ padding: "6px 12px", borderRadius: "15px", border: "none", fontSize: "13px", cursor: "pointer", background: activeTab === pack ? "white" : "rgba(255,255,255,0.1)", color: activeTab === pack ? "black" : "white", transition: "all 0.2s", fontWeight: activeTab === pack ? "bold" : "normal" }}>{pack}</button>
                    ))}
                </div>
            )}
            
            <div style={{ flex: 1, overflowX: "hidden", overflowY: "auto", display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(100px, 1fr))", gridAutoRows: "min-content", gap: "15px", paddingBottom: "10px" }}>
                {displayedStickers.map((s, index) => (
                    <div key={s.path} onClick={() => handleStickerClick(s.path)} style={{ position: "relative", display: "flex", flexDirection: "column", alignItems: "center", padding: "10px", background: index === 0 && query.length > 0 ? "rgba(255, 255, 255, 0.3)" : "rgba(255, 255, 255, 0.1)", borderRadius: "8px", backdropFilter: "blur(5px)", cursor: "pointer", transition: "background 0.2s" }} onMouseEnter={(e) => e.currentTarget.style.background = "rgba(255, 255, 255, 0.2)"} onMouseLeave={(e) => e.currentTarget.style.background = index === 0 && query.length > 0 ? "rgba(255, 255, 255, 0.3)" : "rgba(255, 255, 255, 0.1)"}>
                        <div onClick={(e) => handleToggleFav(e, s.path)} style={{ position: "absolute", top: "5px", right: "5px", width: "24px", height: "24px", borderRadius: "50%", background: "rgba(0,0,0,0.3)", color: s.is_favorite ? "#ff4d4d" : "rgba(255,255,255,0.5)", display: "flex", alignItems: "center", justifyContent: "center", fontSize: "14px", cursor: "pointer", zIndex: 10, transition: "all 0.2s" }} onMouseEnter={(e) => e.currentTarget.style.color = "#ff4d4d"} onMouseLeave={(e) => e.currentTarget.style.color = s.is_favorite ? "#ff4d4d" : "rgba(255,255,255,0.5)"}>♥</div>
                        <img src={convertFileSrc(s.path)} alt={s.name} style={{ width: "80px", height: "80px", objectFit: "contain", marginBottom: "10px" }} />
                        <span style={{ fontSize: "12px", textAlign: "center", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: "100%" }}>{s.name}</span>
                    </div>
                ))}
            </div>
          </>
      )}
    </div>
  );
}

export default App;