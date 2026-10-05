import React from 'react';
import { createRoot } from 'react-dom/client';
import { Titlebar } from './titlebar/titlebar.jsx';
import './styles.css';

createRoot(document.getElementById('root')).render(<Titlebar />);
