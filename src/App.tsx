import { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface Sticker {
  name: string;
  path: string;
  format: string;
  pack: string;
  is_favorite: boolean,
  rec_score: number,
}

function App() {
  const [stickers, setStickers] = useState<Sticker[]>([]);
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState("All");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadStickers("");

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
            .slice(0, 18); // take top 18
    }
    if (activeTab === "Favorites") return stickers.filter(s => s.is_favorite);
    if (activeTab === "All") return stickers;
    return stickers.filter(s => s.pack === activeTab);
  }, [stickers, query, activeTab]);

  const handleToggleFav = async (e: React.MouseEvent, path: string) => {
    e.stopPropagation();
    await invoke("toggle_favorite", { path });
    loadStickers(query);
  };

  return (
    <div style={{ 
      padding: "20px", 
      height: "100%", 
      width: "100%",
      overflowX: "hidden",
      overflowY: "hidden",
      boxSizing: "border-box",
      background: "transparent",
      display: "flex",
      flexDirection: "column",
    }}>
      <h1 style={{ marginBottom: "20px" }}>Soji</h1>
      {/* SEARCH BAR */}
      <input 
        ref={inputRef}
        type="text" 
        placeholder="Search stickers..." 
        value={query}
        onChange={handleSearch}
        onKeyDown={handleKeyDown}
        style={{
            width: "100%",
            padding: "12px",
            marginBottom: "15px",
            borderRadius: "8px",
            border: "1px solid rgba(255,255,255,0.2)",
            background: "rgba(0,0,0,0.3)",
            color: "white",
            fontSize: "16px",
            outline: "none",
            backdropFilter: "blur(10px)"
        }}
      />

      {/* TABS BAR - Only show if not searching */}
      {query.length === 0 && (
          <div style={{
              display: "flex",
              gap: "8px",
              overflowX: "auto",
              paddingBottom: "10px",
              marginBottom: "5px",
              whiteSpace: "nowrap",
          }}>              
              {packs.map(pack => (
                  <button
                    key={pack}
                    onClick={() => setActiveTab(pack)}
                    style={{
                        padding: "6px 12px",
                        borderRadius: "15px",
                        border: "none",
                        fontSize: "13px",
                        cursor: "pointer",
                        background: activeTab === pack ? "white" : "rgba(255,255,255,0.1)",
                        color: activeTab === pack ? "black" : "white",
                        transition: "all 0.2s",
                        fontWeight: activeTab === pack ? "bold" : "normal"
                    }}
                  >
                      {pack}
                  </button>
              ))}
          </div>
      )}
      {/* THE GRID (Scrollable Area) */}
      <div style={{ 
        flex: 1, // Fill remaining space
        overflowX: "hidden",
        overflowY: "auto",
        display: "grid", 
        gridTemplateColumns: "repeat(auto-fill, minmax(100px, 1fr))", 
        gridAutoRows: "min-content",
        gap: "15px",
        paddingBottom: "10px"
      }}>
        {displayedStickers.map((s, index) => (
          <div key={s.path} 
          onClick={() => handleStickerClick(s.path)}
          style={{
            position: "relative",
            display: "flex", 
            flexDirection: "column", 
            alignItems: "center",
            padding: "10px",
            background: index === 0 && query.length > 0 ? "rgba(255, 255, 255, 0.3)" : "rgba(255, 255, 255, 0.1)", // Highlight top result
            borderRadius: "8px",
            backdropFilter: "blur(5px)",
            cursor: "pointer",
            transition: "background 0.2s"
          }}
          onMouseEnter={(e) => e.currentTarget.style.background = "rgba(255, 255, 255, 0.2)"}
          onMouseLeave={(e) => e.currentTarget.style.background = index === 0 && query.length > 0 ? "rgba(255, 255, 255, 0.3)" : "rgba(255, 255, 255, 0.1)"}
          >
            {/* 2. INSERT HEART ICON HERE (Top Right) */}
            <div 
                onClick={(e) => handleToggleFav(e, s.path)}
                style={{
                    position: "absolute",
                    top: "5px",
                    right: "5px",
                    width: "24px",
                    height: "24px",
                    borderRadius: "50%",
                    background: "rgba(0,0,0,0.3)", 
                    color: s.is_favorite ? "#ff4d4d" : "rgba(255,255,255,0.5)", 
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    fontSize: "14px",
                    cursor: "pointer",
                    zIndex: 10,
                    transition: "all 0.2s"
                }}
                onMouseEnter={(e) => e.currentTarget.style.color = "#ff4d4d"}
                onMouseLeave={(e) => e.currentTarget.style.color = s.is_favorite ? "#ff4d4d" : "rgba(255,255,255,0.5)"}
            >
                ♥
            </div>

            <img 
              src={convertFileSrc(s.path)}
              alt={s.name}
              style={{ 
                width: "80px", 
                height: "80px", 
                objectFit: "contain", 
                marginBottom: "10px" 
              }} 
            />
            
            <span style={{ fontSize: "12px", textAlign: "center", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: "100%" }}>
              {s.name}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

export default App;