import React from 'react';
import { IndexingProgress } from '../types';

export const LoadingOverlay: React.FC<{ progress: IndexingProgress }> = ({ progress }) => {
    const formatEta = (seconds: number | null) => {
        if (seconds === null) return "Calculating ETA...";
        if (seconds < 60) return `~${seconds}s remaining`;
        const mins = Math.ceil(seconds / 60);
        return `~${mins}m remaining`;
    };

    return (
        <div style={{
            position: "fixed", top: 0, left: 0, width: "100%", height: "100%",
            background: "rgba(0, 0, 0, 0.75)",
            zIndex: 200,
            display: "flex", alignItems: "center", justifyContent: "center",
            backdropFilter: "blur(20px)", color: "white"
        }}>
            <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: "25px", textShadow: "0 2px 10px rgba(0,0,0,0.5)" }}>
                <div className="spinner"></div>
                <h2 style={{ margin: 0, fontSize: "26px", fontWeight: "400" }}>Indexing Images...</h2>
                <div style={{ fontSize: "20px", opacity: 0.9, fontVariantNumeric: "tabular-nums", fontWeight: "bold" }}>
                    {progress.current} / {progress.total}
                </div>
                <div style={{ fontSize: "14px", opacity: 0.7 }}>
                    {formatEta(progress.eta_seconds)}
                </div>
            </div>
        </div>
    );
};