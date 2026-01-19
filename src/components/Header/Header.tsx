import React from 'react';
import './Header.css';

interface HeaderProps {
    onToggleSettings: () => void;
    onRefresh: () => void;
    onClose: () => void;
}

export const Header: React.FC<HeaderProps> = ({ onToggleSettings, onRefresh, onClose }) => {
    return (
        <div 
            className="header-container"
            data-tauri-drag-region 
        >
            <h1 className="header-title">Soji</h1>
            
            <div className="header-controls">
                <button className="header-btn" onClick={onRefresh} title="Refresh Library">↻</button>
                <button className="header-btn" onClick={onToggleSettings} title="Settings">⚙</button>
                <button className="header-btn close" onClick={onClose} title="Close">✕</button>
            </div>
        </div>
    );
};