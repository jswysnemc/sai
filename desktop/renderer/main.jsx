import React from 'react';
import { createRoot } from 'react-dom/client';
import { WindowControls } from './window-controls/window-controls.jsx';
import './styles.css';

createRoot(document.getElementById('root')).render(<WindowControls />);
