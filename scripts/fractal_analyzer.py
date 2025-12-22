#!/usr/bin/env python3
"""
Fractal Image Complexity Analyzer

Analyzes PNG images to detect and quantify complex fractal patterns using
multiple metrics including FFT power spectrum analysis, entropy measures,
fractal dimension, and lacunarity.
"""

import tkinter as tk
from tkinter import ttk, filedialog, messagebox
import numpy as np
from PIL import Image, ImageTk
import io
import zlib
from pathlib import Path


class FractalAnalyzer:
    """Computes various complexity metrics for fractal images."""
    
    def __init__(self, image_array: np.ndarray):
        """
        Initialize with a grayscale image array.
        
        Args:
            image_array: 2D numpy array of pixel values (0-255 or 0-1)
        """
        # Normalize to 0-1 range
        self.image = image_array.astype(np.float64)
        if self.image.max() > 1:
            self.image = self.image / 255.0
        
        self.height, self.width = self.image.shape
        self._fft_cache = None
        self._radial_cache = None
    
    def compute_fft(self) -> tuple[np.ndarray, np.ndarray]:
        """
        Compute 2D FFT and return shifted magnitude spectrum.
        
        Returns:
            (magnitude, phase) both centered with DC at middle
        """
        if self._fft_cache is not None:
            return self._fft_cache
        
        f_transform = np.fft.fft2(self.image)
        f_shift = np.fft.fftshift(f_transform)
        magnitude = np.abs(f_shift)
        phase = np.angle(f_shift)
        
        self._fft_cache = (magnitude, phase)
        return magnitude, phase
    
    def radial_power_profile(self) -> tuple[np.ndarray, np.ndarray]:
        """
        Compute radially averaged power spectrum.
        
        Returns:
            (frequencies, power) arrays for plotting
        """
        if self._radial_cache is not None:
            return self._radial_cache
        
        magnitude, _ = self.compute_fft()
        power = magnitude ** 2
        
        center_y, center_x = self.height // 2, self.width // 2
        y, x = np.ogrid[:self.height, :self.width]
        r = np.sqrt((x - center_x)**2 + (y - center_y)**2).astype(int)
        
        max_r = min(center_x, center_y)
        r_flat = r.ravel()
        power_flat = power.ravel()
        
        # Filter to valid radius range
        mask = r_flat < max_r
        r_masked = r_flat[mask]
        power_masked = power_flat[mask]
        
        # Radial average
        radial_sum = np.bincount(r_masked, weights=power_masked)
        radial_count = np.bincount(r_masked)
        radial_count[radial_count == 0] = 1  # avoid division by zero
        radial_power = radial_sum / radial_count
        
        frequencies = np.arange(len(radial_power))
        
        self._radial_cache = (frequencies, radial_power)
        return frequencies, radial_power
    
    def power_spectrum_slope(self) -> tuple[float, float, np.ndarray, np.ndarray]:
        """
        Fit power-law slope to radial power spectrum.
        
        Returns:
            (slope_beta, r_squared, log_freq, log_power) for the fit
        """
        freq, power = self.radial_power_profile()
        
        # Skip DC component and very high frequencies (noise)
        min_f, max_f = 2, len(freq) // 2
        freq_range = freq[min_f:max_f]
        power_range = power[min_f:max_f]
        
        # Filter zeros for log
        mask = power_range > 0
        freq_range = freq_range[mask]
        power_range = power_range[mask]
        
        if len(freq_range) < 10:
            return 0.0, 0.0, np.array([]), np.array([])
        
        log_freq = np.log10(freq_range)
        log_power = np.log10(power_range)
        
        # Linear regression
        coeffs = np.polyfit(log_freq, log_power, 1)
        slope = coeffs[0]
        
        # R-squared
        predicted = np.polyval(coeffs, log_freq)
        ss_res = np.sum((log_power - predicted) ** 2)
        ss_tot = np.sum((log_power - np.mean(log_power)) ** 2)
        r_squared = 1 - (ss_res / ss_tot) if ss_tot > 0 else 0
        
        return -slope, r_squared, log_freq, log_power  # negate for β convention
    
    def shannon_entropy(self) -> float:
        """Compute Shannon entropy of pixel value distribution."""
        # Quantize to 256 levels
        quantized = (self.image * 255).astype(np.uint8)
        hist, _ = np.histogram(quantized, bins=256, range=(0, 255))
        hist = hist[hist > 0]  # remove zeros
        probs = hist / hist.sum()
        return -np.sum(probs * np.log2(probs))
    
    def local_entropy_variance(self, window_size: int = 16) -> float:
        """
        Compute variance of local entropy across image patches.
        High variance suggests interesting multi-scale structure.
        """
        entropies = []
        for y in range(0, self.height - window_size, window_size):
            for x in range(0, self.width - window_size, window_size):
                patch = self.image[y:y+window_size, x:x+window_size]
                quantized = (patch * 255).astype(np.uint8)
                hist, _ = np.histogram(quantized, bins=64, range=(0, 255))
                hist = hist[hist > 0]
                if len(hist) > 1:
                    probs = hist / hist.sum()
                    entropy = -np.sum(probs * np.log2(probs))
                    entropies.append(entropy)
        
        return np.var(entropies) if entropies else 0.0
    
    def compression_ratio(self) -> float:
        """
        Estimate Kolmogorov complexity via compression ratio.
        Lower ratio = more complex/random, higher = more compressible/simple.
        """
        raw_data = (self.image * 255).astype(np.uint8).tobytes()
        compressed = zlib.compress(raw_data, level=9)
        return len(compressed) / len(raw_data)
    
    def box_counting_dimension(self, min_box: int = 2, max_box: int = 64) -> tuple[float, float]:
        """
        Estimate fractal dimension using box-counting method.
        
        Returns:
            (dimension, r_squared) of the fit
        """
        # Binarize at median
        threshold = np.median(self.image)
        binary = self.image > threshold
        
        sizes = []
        counts = []
        
        box_size = min_box
        while box_size <= max_box and box_size <= min(self.height, self.width) // 2:
            count = 0
            for y in range(0, self.height, box_size):
                for x in range(0, self.width, box_size):
                    box = binary[y:y+box_size, x:x+box_size]
                    if np.any(box):
                        count += 1
            
            if count > 0:
                sizes.append(box_size)
                counts.append(count)
            
            box_size *= 2
        
        if len(sizes) < 3:
            return 0.0, 0.0
        
        log_sizes = np.log(1.0 / np.array(sizes))
        log_counts = np.log(np.array(counts))
        
        coeffs = np.polyfit(log_sizes, log_counts, 1)
        dimension = coeffs[0]
        
        predicted = np.polyval(coeffs, log_sizes)
        ss_res = np.sum((log_counts - predicted) ** 2)
        ss_tot = np.sum((log_counts - np.mean(log_counts)) ** 2)
        r_squared = 1 - (ss_res / ss_tot) if ss_tot > 0 else 0
        
        return dimension, r_squared
    
    def lacunarity(self, box_sizes: list[int] = None) -> tuple[float, list[float]]:
        """
        Compute lacunarity (gappiness) at multiple scales.
        
        Returns:
            (mean_lacunarity, lacunarity_per_scale)
        """
        if box_sizes is None:
            box_sizes = [4, 8, 16, 32]
        
        # Binarize
        threshold = np.median(self.image)
        binary = (self.image > threshold).astype(np.float64)
        
        lacunarities = []
        
        for box_size in box_sizes:
            if box_size > min(self.height, self.width) // 2:
                continue
            
            masses = []
            for y in range(0, self.height - box_size + 1, box_size // 2):
                for x in range(0, self.width - box_size + 1, box_size // 2):
                    box = binary[y:y+box_size, x:x+box_size]
                    masses.append(np.sum(box))
            
            masses = np.array(masses)
            if len(masses) > 1 and np.mean(masses) > 0:
                # Lacunarity = variance/mean^2 + 1
                lac = np.var(masses) / (np.mean(masses) ** 2) + 1
                lacunarities.append(lac)
        
        mean_lac = np.mean(lacunarities) if lacunarities else 1.0
        return mean_lac, lacunarities
    
    def edge_density(self) -> float:
        """Compute edge density using Sobel-like gradient magnitude."""
        # Simple gradient
        gy = np.diff(self.image, axis=0)
        gx = np.diff(self.image, axis=1)
        
        # Magnitude (trimmed to same size)
        min_h = min(gy.shape[0], gx.shape[0])
        min_w = min(gy.shape[1], gx.shape[1])
        
        gy_trim = gy[:min_h, :min_w]
        gx_trim = gx[:min_h, :min_w]
        
        gradient_mag = np.sqrt(gy_trim**2 + gx_trim**2)
        return np.mean(gradient_mag)
    
    def dynamic_range_usage(self) -> tuple[float, float, float]:
        """
        Analyze how well the image uses its dynamic range.
        
        Returns:
            (range_fraction, percentile_5, percentile_95)
        """
        p5 = np.percentile(self.image, 5)
        p95 = np.percentile(self.image, 95)
        range_fraction = p95 - p5
        return range_fraction, p5, p95
    
    def spectral_flatness(self) -> float:
        """
        Compute spectral flatness (Wiener entropy).
        1.0 = white noise, 0.0 = pure tone/very structured
        """
        _, power = self.radial_power_profile()
        power = power[1:]  # skip DC
        power = power[power > 0]
        
        if len(power) == 0:
            return 0.0
        
        geometric_mean = np.exp(np.mean(np.log(power)))
        arithmetic_mean = np.mean(power)
        
        return geometric_mean / arithmetic_mean if arithmetic_mean > 0 else 0.0
    
    def analyze_all(self) -> dict:
        """Run all analyses and return results dictionary."""
        beta, beta_r2, _, _ = self.power_spectrum_slope()
        box_dim, box_r2 = self.box_counting_dimension()
        mean_lac, lac_scales = self.lacunarity()
        dyn_range, p5, p95 = self.dynamic_range_usage()
        
        return {
            'power_spectrum_slope': beta,
            'power_spectrum_r2': beta_r2,
            'shannon_entropy': self.shannon_entropy(),
            'local_entropy_variance': self.local_entropy_variance(),
            'compression_ratio': self.compression_ratio(),
            'box_counting_dimension': box_dim,
            'box_counting_r2': box_r2,
            'mean_lacunarity': mean_lac,
            'lacunarity_per_scale': lac_scales,
            'edge_density': self.edge_density(),
            'dynamic_range': dyn_range,
            'spectral_flatness': self.spectral_flatness(),
        }
    
    def complexity_score(self) -> tuple[float, dict]:
        """
        Compute overall complexity score (0-100) combining metrics.
        
        Returns:
            (score, component_scores)
        """
        results = self.analyze_all()
        
        components = {}
        
        # Power spectrum slope: ideal range 1.0-2.5, peak around 1.5-2.0
        beta = results['power_spectrum_slope']
        if 1.0 <= beta <= 2.5:
            beta_score = 100 - 40 * abs(beta - 1.75)
        else:
            beta_score = max(0, 50 - 30 * min(abs(beta - 1.0), abs(beta - 2.5)))
        components['spectrum_slope'] = max(0, min(100, beta_score))
        
        # Entropy: higher is more complex (max ~8 for 8-bit)
        entropy = results['shannon_entropy']
        components['entropy'] = min(100, entropy * 12.5)
        
        # Box dimension: ideal 1.5-2.0 for interesting 2D fractals
        dim = results['box_counting_dimension']
        if 1.3 <= dim <= 2.0:
            dim_score = 100 - 50 * abs(dim - 1.7)
        else:
            dim_score = max(0, 40 - 40 * min(abs(dim - 1.3), abs(dim - 2.0)))
        components['fractal_dimension'] = max(0, min(100, dim_score))
        
        # Lacunarity: moderate values suggest interesting texture
        lac = results['mean_lacunarity']
        if 1.1 <= lac <= 3.0:
            lac_score = 80
        elif lac > 3.0:
            lac_score = max(20, 80 - 10 * (lac - 3.0))
        else:
            lac_score = max(20, lac * 70)
        components['lacunarity'] = min(100, lac_score)
        
        # Dynamic range: using full range is good
        dyn = results['dynamic_range']
        components['dynamic_range'] = min(100, dyn * 120)
        
        # Compression ratio: 0.3-0.7 suggests good complexity
        comp = results['compression_ratio']
        if 0.3 <= comp <= 0.7:
            comp_score = 90
        elif comp < 0.3:
            comp_score = comp * 200  # too random
        else:
            comp_score = max(30, 90 - 100 * (comp - 0.7))  # too compressible
        components['compression'] = min(100, comp_score)
        
        # Local entropy variance: some variance is good
        lev = results['local_entropy_variance']
        lev_score = min(100, lev * 200)
        components['local_entropy_var'] = lev_score
        
        # Weighted average
        weights = {
            'spectrum_slope': 0.20,
            'entropy': 0.15,
            'fractal_dimension': 0.20,
            'lacunarity': 0.10,
            'dynamic_range': 0.10,
            'compression': 0.15,
            'local_entropy_var': 0.10,
        }
        
        total_score = sum(components[k] * weights[k] for k in weights)
        
        return total_score, components


class AnalyzerGUI:
    """Tkinter GUI for the fractal analyzer."""
    
    def __init__(self, root: tk.Tk):
        self.root = root
        self.root.title("Fractal Complexity Analyzer")
        self.root.geometry("1400x900")
        
        self.image_array = None
        self.analyzer = None
        self.photo_refs = []  # prevent garbage collection
        
        self._build_ui()
    
    def _build_ui(self):
        # Main container
        main_frame = ttk.Frame(self.root, padding="10")
        main_frame.grid(row=0, column=0, sticky="nsew")
        
        self.root.columnconfigure(0, weight=1)
        self.root.rowconfigure(0, weight=1)
        main_frame.columnconfigure(1, weight=1)
        main_frame.rowconfigure(1, weight=1)
        
        # Top controls
        control_frame = ttk.Frame(main_frame)
        control_frame.grid(row=0, column=0, columnspan=2, sticky="ew", pady=(0, 10))
        
        ttk.Button(control_frame, text="Open Image", command=self._open_image).pack(side="left", padx=5)
        ttk.Button(control_frame, text="Analyze", command=self._run_analysis).pack(side="left", padx=5)
        ttk.Button(control_frame, text="Export Report", command=self._export_report).pack(side="left", padx=5)
        
        self.status_var = tk.StringVar(value="No image loaded")
        ttk.Label(control_frame, textvariable=self.status_var).pack(side="right", padx=10)
        
        # Left panel: images
        left_frame = ttk.LabelFrame(main_frame, text="Visualizations", padding="5")
        left_frame.grid(row=1, column=0, sticky="nsew", padx=(0, 5))
        
        # Image display
        img_frame = ttk.Frame(left_frame)
        img_frame.pack(fill="both", expand=True)
        
        # Original image
        orig_container = ttk.LabelFrame(img_frame, text="Original Image")
        orig_container.pack(side="left", fill="both", expand=True, padx=2)
        self.original_canvas = tk.Canvas(orig_container, width=300, height=300, bg='#1a1a1a')
        self.original_canvas.pack(fill="both", expand=True)
        
        # FFT magnitude
        fft_container = ttk.LabelFrame(img_frame, text="FFT Magnitude (log)")
        fft_container.pack(side="left", fill="both", expand=True, padx=2)
        self.fft_canvas = tk.Canvas(fft_container, width=300, height=300, bg='#1a1a1a')
        self.fft_canvas.pack(fill="both", expand=True)
        
        # Power spectrum plot
        plot_container = ttk.LabelFrame(left_frame, text="Radial Power Spectrum")
        plot_container.pack(fill="both", expand=True, pady=(5, 0))
        self.plot_canvas = tk.Canvas(plot_container, width=600, height=250, bg='white')
        self.plot_canvas.pack(fill="both", expand=True)
        
        # Right panel: metrics
        right_frame = ttk.LabelFrame(main_frame, text="Analysis Results", padding="10")
        right_frame.grid(row=1, column=1, sticky="nsew", padx=(5, 0))
        main_frame.columnconfigure(1, weight=1)
        
        # Scrollable metrics area
        metrics_canvas = tk.Canvas(right_frame)
        scrollbar = ttk.Scrollbar(right_frame, orient="vertical", command=metrics_canvas.yview)
        self.metrics_frame = ttk.Frame(metrics_canvas)
        
        self.metrics_frame.bind(
            "<Configure>",
            lambda e: metrics_canvas.configure(scrollregion=metrics_canvas.bbox("all"))
        )
        
        metrics_canvas.create_window((0, 0), window=self.metrics_frame, anchor="nw")
        metrics_canvas.configure(yscrollcommand=scrollbar.set)
        
        metrics_canvas.pack(side="left", fill="both", expand=True)
        scrollbar.pack(side="right", fill="y")
        
        # Placeholder text
        ttk.Label(self.metrics_frame, text="Load an image and click Analyze").pack(pady=20)
    
    def _open_image(self):
        filepath = filedialog.askopenfilename(
            filetypes=[("PNG files", "*.png"), ("All image files", "*.png *.jpg *.jpeg *.bmp *.gif")]
        )
        if not filepath:
            return
        
        try:
            img = Image.open(filepath)
            # Convert to grayscale
            if img.mode != 'L':
                img = img.convert('L')
            
            self.image_array = np.array(img)
            self.analyzer = FractalAnalyzer(self.image_array)
            
            self._display_original(img)
            self.status_var.set(f"Loaded: {Path(filepath).name} ({img.size[0]}x{img.size[1]})")
            
            # Clear previous results
            for widget in self.metrics_frame.winfo_children():
                widget.destroy()
            ttk.Label(self.metrics_frame, text="Click 'Analyze' to compute metrics").pack(pady=20)
            
        except Exception as e:
            messagebox.showerror("Error", f"Failed to load image: {e}")
    
    def _display_original(self, img: Image.Image):
        # Resize to fit canvas
        canvas_size = 300
        img_display = img.copy()
        img_display.thumbnail((canvas_size, canvas_size), Image.Resampling.LANCZOS)
        
        photo = ImageTk.PhotoImage(img_display)
        self.photo_refs.append(photo)
        
        self.original_canvas.delete("all")
        self.original_canvas.create_image(
            canvas_size // 2, canvas_size // 2,
            image=photo, anchor="center"
        )
    
    def _display_fft(self):
        if self.analyzer is None:
            return
        
        magnitude, _ = self.analyzer.compute_fft()
        
        # Log scale for visualization
        log_mag = np.log1p(magnitude)
        
        # Normalize to 0-255
        log_mag = (log_mag - log_mag.min()) / (log_mag.max() - log_mag.min() + 1e-10)
        log_mag = (log_mag * 255).astype(np.uint8)
        
        img = Image.fromarray(log_mag, mode='L')
        
        canvas_size = 300
        img.thumbnail((canvas_size, canvas_size), Image.Resampling.LANCZOS)
        
        photo = ImageTk.PhotoImage(img)
        self.photo_refs.append(photo)
        
        self.fft_canvas.delete("all")
        self.fft_canvas.create_image(
            canvas_size // 2, canvas_size // 2,
            image=photo, anchor="center"
        )
    
    def _draw_power_spectrum_plot(self):
        if self.analyzer is None:
            return
        
        self.plot_canvas.delete("all")
        
        beta, r2, log_freq, log_power = self.analyzer.power_spectrum_slope()
        
        if len(log_freq) == 0:
            self.plot_canvas.create_text(300, 125, text="Insufficient data for plot", fill="gray")
            return
        
        # Canvas dimensions
        width = self.plot_canvas.winfo_width() or 600
        height = self.plot_canvas.winfo_height() or 250
        
        margin = 50
        plot_w = width - 2 * margin
        plot_h = height - 2 * margin
        
        # Draw axes
        self.plot_canvas.create_line(margin, height - margin, width - margin, height - margin, fill="black")
        self.plot_canvas.create_line(margin, margin, margin, height - margin, fill="black")
        
        # Scale data to plot area
        x_min, x_max = log_freq.min(), log_freq.max()
        y_min, y_max = log_power.min(), log_power.max()
        
        def scale_x(x):
            return margin + (x - x_min) / (x_max - x_min + 1e-10) * plot_w
        
        def scale_y(y):
            return height - margin - (y - y_min) / (y_max - y_min + 1e-10) * plot_h
        
        # Plot data points
        for i in range(len(log_freq)):
            x = scale_x(log_freq[i])
            y = scale_y(log_power[i])
            self.plot_canvas.create_oval(x-2, y-2, x+2, y+2, fill="#3b82f6", outline="")
        
        # Plot fit line
        fit_y = log_power.mean() - beta * (log_freq - log_freq.mean())
        x1, y1 = scale_x(log_freq[0]), scale_y(fit_y[0])
        x2, y2 = scale_x(log_freq[-1]), scale_y(fit_y[-1])
        self.plot_canvas.create_line(x1, y1, x2, y2, fill="#ef4444", width=2)
        
        # Labels
        self.plot_canvas.create_text(width // 2, height - 10, text="log₁₀(frequency)", fill="black")
        self.plot_canvas.create_text(15, height // 2, text="log₁₀(power)", fill="black", angle=90)
        self.plot_canvas.create_text(
            width - margin - 80, margin + 20,
            text=f"β = {beta:.3f}\nR² = {r2:.3f}",
            fill="#ef4444", anchor="ne", justify="right"
        )
    
    def _run_analysis(self):
        if self.analyzer is None:
            messagebox.showwarning("Warning", "Please load an image first")
            return
        
        self.status_var.set("Analyzing...")
        self.root.update()
        
        try:
            # Display FFT
            self._display_fft()
            
            # Draw power spectrum
            self.root.after(10, self._draw_power_spectrum_plot)
            
            # Compute all metrics
            results = self.analyzer.analyze_all()
            score, components = self.analyzer.complexity_score()
            
            # Clear and populate metrics frame
            for widget in self.metrics_frame.winfo_children():
                widget.destroy()
            
            # Overall score
            score_frame = ttk.LabelFrame(self.metrics_frame, text="Overall Complexity Score", padding="10")
            score_frame.pack(fill="x", pady=(0, 10))
            
            score_color = "#22c55e" if score >= 70 else "#eab308" if score >= 40 else "#ef4444"
            score_label = tk.Label(
                score_frame, text=f"{score:.1f} / 100",
                font=("Helvetica", 24, "bold"), fg=score_color
            )
            score_label.pack()
            
            # Interpretation
            if score >= 70:
                interp = "High complexity - interesting fractal patterns detected"
            elif score >= 40:
                interp = "Moderate complexity - some interesting structure"
            else:
                interp = "Low complexity - may be degenerate or too simple"
            ttk.Label(score_frame, text=interp, wraplength=300).pack()
            
            # Component scores
            comp_frame = ttk.LabelFrame(self.metrics_frame, text="Component Scores", padding="10")
            comp_frame.pack(fill="x", pady=(0, 10))
            
            for name, value in components.items():
                row = ttk.Frame(comp_frame)
                row.pack(fill="x", pady=2)
                ttk.Label(row, text=f"{name.replace('_', ' ').title()}:", width=20, anchor="w").pack(side="left")
                
                # Progress bar style score
                bar_frame = ttk.Frame(row)
                bar_frame.pack(side="left", fill="x", expand=True)
                
                bar_canvas = tk.Canvas(bar_frame, height=16, bg="#e5e7eb", highlightthickness=0)
                bar_canvas.pack(fill="x", padx=(0, 10))
                bar_canvas.update()
                bar_width = bar_canvas.winfo_width() or 150
                fill_width = int(bar_width * value / 100)
                bar_color = "#22c55e" if value >= 70 else "#eab308" if value >= 40 else "#ef4444"
                bar_canvas.create_rectangle(0, 0, fill_width, 16, fill=bar_color, outline="")
                
                ttk.Label(row, text=f"{value:.1f}", width=6).pack(side="right")
            
            # Detailed metrics
            detail_frame = ttk.LabelFrame(self.metrics_frame, text="Detailed Metrics", padding="10")
            detail_frame.pack(fill="x", pady=(0, 10))
            
            metrics_display = [
                ("Power Spectrum Slope (β)", f"{results['power_spectrum_slope']:.3f}", 
                 "Ideal: 1.0-2.5 for complex fractals"),
                ("Spectrum Fit R²", f"{results['power_spectrum_r2']:.3f}",
                 "Higher = cleaner power-law behavior"),
                ("Shannon Entropy", f"{results['shannon_entropy']:.3f} bits",
                 "Max ~8 for 8-bit images"),
                ("Local Entropy Variance", f"{results['local_entropy_variance']:.4f}",
                 "Higher = more multi-scale variation"),
                ("Compression Ratio", f"{results['compression_ratio']:.3f}",
                 "0.3-0.7 suggests good complexity"),
                ("Box-Counting Dimension", f"{results['box_counting_dimension']:.3f}",
                 "Ideal: 1.5-2.0 for 2D fractals"),
                ("Dimension Fit R²", f"{results['box_counting_r2']:.3f}",
                 "Higher = cleaner fractal scaling"),
                ("Mean Lacunarity", f"{results['mean_lacunarity']:.3f}",
                 "Texture gappiness measure"),
                ("Edge Density", f"{results['edge_density']:.4f}",
                 "Gradient magnitude average"),
                ("Dynamic Range", f"{results['dynamic_range']:.3f}",
                 "Fraction of intensity range used"),
                ("Spectral Flatness", f"{results['spectral_flatness']:.4f}",
                 "1.0 = white noise, 0.0 = structured"),
            ]
            
            for name, value, desc in metrics_display:
                row = ttk.Frame(detail_frame)
                row.pack(fill="x", pady=3)
                ttk.Label(row, text=name + ":", font=("Helvetica", 9, "bold")).pack(anchor="w")
                ttk.Label(row, text=f"{value}  —  {desc}", foreground="gray").pack(anchor="w", padx=(10, 0))
            
            self.results = results
            self.score = score
            self.components = components
            
            self.status_var.set("Analysis complete")
            
        except Exception as e:
            messagebox.showerror("Error", f"Analysis failed: {e}")
            self.status_var.set("Analysis failed")
            raise
    
    def _export_report(self):
        if not hasattr(self, 'results'):
            messagebox.showwarning("Warning", "Run analysis first")
            return
        
        filepath = filedialog.asksaveasfilename(
            defaultextension=".txt",
            filetypes=[("Text files", "*.txt"), ("All files", "*.*")]
        )
        if not filepath:
            return
        
        with open(filepath, 'w') as f:
            f.write("FRACTAL COMPLEXITY ANALYSIS REPORT\n")
            f.write("=" * 40 + "\n\n")
            f.write(f"Overall Complexity Score: {self.score:.1f} / 100\n\n")
            
            f.write("Component Scores:\n")
            for name, value in self.components.items():
                f.write(f"  {name}: {value:.1f}\n")
            
            f.write("\nDetailed Metrics:\n")
            for key, value in self.results.items():
                if isinstance(value, list):
                    f.write(f"  {key}: {[f'{v:.3f}' for v in value]}\n")
                elif isinstance(value, float):
                    f.write(f"  {key}: {value:.4f}\n")
                else:
                    f.write(f"  {key}: {value}\n")
        
        messagebox.showinfo("Export", f"Report saved to {filepath}")


def main():
    root = tk.Tk()
    app = AnalyzerGUI(root)
    root.mainloop()


if __name__ == "__main__":
    main()
