// DOM element references and shared UI state.
// Loaded first: every other script relies on these globals.

// Elements
const lblState = document.getElementById('lblState');
const lblError = document.getElementById('lblError');
const valVoltage = document.getElementById('valVoltage');
const valCurrent = document.getElementById('valCurrent');
const valPower = document.getElementById('valPower');
const valEnergy = document.getElementById('valEnergy');
const slider = document.getElementById('currentSlider');
const lblSlider = document.getElementById('lblSlider');
const btnEnableDisable = document.getElementById('btnEnableDisable');
const btnPauseResume = document.getElementById('btnPauseResume');
const controlsPanel = document.getElementById('controlsPanel');

// Visualizer Elements
const chargerBox = document.getElementById('chargerBox');
const chargerIcon = document.getElementById('chargerIcon');
const chargerLed = document.getElementById('chargerLed');
const carIcon = document.getElementById('carIcon');
const carBox = document.getElementById('carBox');

// Linky
const linkyBox = document.getElementById('linkyBox');
const linkyContainer = document.getElementById('linkyContainer');
const linkyLed = document.getElementById('linkyLed');
const linkyLabel = document.getElementById('linkyLabel');
const linkyIcon = document.getElementById('linkyIcon');
const animWireLinky = document.getElementById('animWireLinky');

// Header & BT
const btToggle = document.getElementById('btToggle');
const boardInfo = document.getElementById('boardInfo');
const subInfo = document.getElementById('subInfo');

// Shared state
let isActionPending = false;
let isDraggingSlider = false;
let lastReportedState = "";
let lastReportedTicMode = null;
window.lastChargeAuthorized = false;
window.lastChargePaused = false;
