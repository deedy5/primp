#!/usr/bin/env python3
"""
Simple benchmark runner.

Creates an isolated virtual environment, installs dependencies,
runs the benchmark server, executes benchmarks, and cleans up.

Usage:
    python run.py
"""

import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

BENCHMARK_DIR = Path(__file__).parent
VENV_DIR = BENCHMARK_DIR / ".venv_benchmark"
DEFAULT_PORT = 8000
HOST = "127.0.0.1"
STARTUP_TIMEOUT = 30


def create_venv() -> Path:
    """Create a virtual environment and return the path to its python executable."""
    print(f"Creating virtual environment at {VENV_DIR}...")

    # Remove existing venv if present
    if VENV_DIR.exists():
        print("Removing existing virtual environment...")
        shutil.rmtree(VENV_DIR)

    # Create new venv
    subprocess.run(
        [sys.executable, "-m", "venv", str(VENV_DIR)],
        check=True,
    )

    # Determine python executable path in venv
    if sys.platform == "win32":
        python_path = VENV_DIR / "Scripts" / "python.exe"
    else:
        python_path = VENV_DIR / "bin" / "python"

    print("Virtual environment created.")
    return python_path


def install_dependencies(python_path: Path):
    """Install dependencies from requirements.txt."""
    print("Installing dependencies...")

    requirements_path = BENCHMARK_DIR / "requirements.txt"

    subprocess.run(
        [str(python_path), "-m", "pip", "install", "--upgrade", "pip"],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    subprocess.run(
        [str(python_path), "-m", "pip", "install", "-r", str(requirements_path)],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    print("Dependencies installed.")


def is_server_ready(port: int, timeout: float = STARTUP_TIMEOUT) -> bool:
    """Wait for the server to be ready to accept connections."""
    start_time = time.perf_counter()
    while time.perf_counter() - start_time < timeout:
        try:
            with socket.create_connection((HOST, port), timeout=0.5):
                return True
        except OSError:
            time.sleep(0.2)
    return False


def start_server(port: int, python_path: Path):
    """Start the server as a subprocess."""
    print(f"Starting benchmark server on {HOST}:{port}...")

    server_process = subprocess.Popen(
        [
            str(python_path),
            "-m",
            "granian",
            "server:app",
            "--host",
            HOST,
            "--port",
            str(port),
            "--interface",
            "asgi",
            "--workers",
            "1",
            "--log-level",
            "error",
        ],
        cwd=str(BENCHMARK_DIR),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    if not is_server_ready(port):
        print("ERROR: Server failed to start")
        server_process.kill()
        sys.exit(1)

    print("Server is ready.")
    return server_process


def run_benchmark(python_path: Path):
    """Run the benchmark."""
    print("\nRunning benchmark...\n")

    result = subprocess.run(
        [str(python_path), str(BENCHMARK_DIR / "benchmark.py")],
        cwd=str(BENCHMARK_DIR),
    )

    return result.returncode


def cleanup_venv():
    """Remove the virtual environment."""
    if VENV_DIR.exists():
        print(f"\nCleaning up virtual environment at {VENV_DIR}...")
        shutil.rmtree(VENV_DIR)
        print("Cleanup complete.")


def main():
    """Main function."""
    port = DEFAULT_PORT
    server_process = None
    python_path = None

    try:
        # Create virtual environment
        python_path = create_venv()

        # Install dependencies
        install_dependencies(python_path)

        # Start server
        server_process = start_server(port, python_path)

        # Run benchmark (includes image generation)
        return_code = run_benchmark(python_path)

        if return_code != 0:
            sys.exit(1)

    except KeyboardInterrupt:
        print("\nInterrupted by user")
        sys.exit(1)
    finally:
        # Stop server
        if server_process:
            server_process.terminate()
            try:
                server_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server_process.kill()

        # Cleanup virtual environment
        cleanup_venv()


if __name__ == "__main__":
    main()
