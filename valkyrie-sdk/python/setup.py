#!/usr/bin/env python3
"""
Valkyrie Protocol Python SDK Setup
"""

from setuptools import setup, find_packages
import os

# Read the README file
def read_readme():
    with open("README.md", "r", encoding="utf-8") as fh:
        return fh.read()

# Read requirements
def read_requirements():
    with open("requirements.txt", "r", encoding="utf-8") as fh:
        return [line.strip() for line in fh if line.strip() and not line.startswith("#")]

setup(
    name="valkyrie-protocol",
    version="0.1.0",
    author="RustCI Team",
    author_email="team@rustci.dev",
    description="Python SDK for Valkyrie Protocol - High-performance distributed messaging",
    long_description=read_readme(),
    long_description_content_type="text/markdown",
    url="https://github.com/rustci/valkyrie-protocol",
    project_urls={
        "Bug Tracker": "https://github.com/rustci/valkyrie-protocol/issues",
        "Documentation": "https://docs.valkyrie-protocol.dev",
        "Source Code": "https://github.com/rustci/valkyrie-protocol",
    },
    packages=find_packages(where="src"),
    package_dir={"": "src"},
    classifiers=[
        "Development Status :: 4 - Beta",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "License :: OSI Approved :: Apache Software License",
        "Operating System :: OS Independent",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Topic :: Software Development :: Libraries :: Python Modules",
        "Topic :: System :: Networking",
        "Topic :: Internet",
    ],
    python_requires=">=3.8",
    install_requires=read_requirements(),
    extras_require={
        "dev": [
            "pytest>=7.0",
            "pytest-asyncio>=0.21",
            "pytest-cov>=4.0",
            "black>=23.0",
            "isort>=5.0",
            "mypy>=1.0",
            "flake8>=6.0",
        ],
        "docs": [
            "sphinx>=5.0",
            "sphinx-rtd-theme>=1.0",
            "sphinx-autodoc-typehints>=1.0",
        ],
    },
    entry_points={
        "console_scripts": [
            "valkyrie-client=valkyrie.cli.client:main",
            "valkyrie-server=valkyrie.cli.server:main",
            "valkyrie-benchmark=valkyrie.cli.benchmark:main",
        ],
    },
    keywords=[
        "valkyrie", "protocol", "networking", "async", "distributed",
        "messaging", "rpc", "sdk", "client", "server"
    ],
    zip_safe=False,
    include_package_data=True,
)