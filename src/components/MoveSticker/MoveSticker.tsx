import React, { useState } from 'react';
import '../Settings/Settings.css';
import './MoveSticker.css';

interface MoveStickerModalProps {
    stickerName: string;
    currentPack: string;
    packs: string[];
    onClose: () => void;
    onMove: (packName: string) => void;
}

export const MoveStickerModal: React.FC<MoveStickerModalProps> = ({ 
    stickerName, currentPack, packs, onClose, onMove 
}) => {
    const [selectedPack, setSelectedPack] = useState<string | null>(null);
    const [newPackName, setNewPackName] = useState("");

    const handleCreateAndMove = () => {
        if (newPackName.trim()) {
            onMove(newPackName.trim());
        }
    };

    return (
        <div className="settings-overlay" onClick={onClose} onContextMenu={(e) => e.preventDefault()}>
            <div className="settings-modal" onClick={e => e.stopPropagation()}>
                <h3 className="settings-header" style={{fontSize: '18px'}}>
                    Move "{stickerName}"
                </h3>

                <div className="settings-section">
                    <label className="settings-label">Select Existing Pack</label>
                    <div className="move-modal-list">
                        {packs.filter(p => p !== currentPack).map(pack => (
                            <button 
                                key={pack}
                                className={`pack-item-btn ${selectedPack === pack ? 'active' : ''}`}
                                onClick={() => {
                                    setSelectedPack(pack);
                                    onMove(pack);
                                }}
                            >
                                📁 {pack}
                            </button>
                        ))}
                        {packs.length <= 1 && (
                            <div style={{padding: '10px', opacity: 0.5, fontSize: '13px', textAlign: 'center'}}>
                                No other packs found.
                            </div>
                        )}
                    </div>
                </div>

                <div className="settings-section">
                    <label className="settings-label">Or Create New Pack</label>
                    <div className="new-pack-input-group">
                        <input 
                            type="text" 
                            placeholder="New Folder Name..."
                            className="settings-input-readonly"
                            style={{background: 'rgba(0,0,0,0.3)', cursor: 'text'}}
                            value={newPackName}
                            onChange={(e) => setNewPackName(e.target.value)}
                            onKeyDown={(e) => e.key === 'Enter' && handleCreateAndMove()}
                            autoFocus
                        />
                        <button 
                            className="settings-btn-primary"
                            onClick={handleCreateAndMove}
                            disabled={!newPackName.trim()}
                            style={{opacity: newPackName.trim() ? 1 : 0.5}}
                        >
                            Move
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
};