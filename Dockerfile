# Accept registry as build argument (not sensitive)
ARG REGISTRY="docker.io"
FROM ${REGISTRY}/ubuntu:22.04

# Copy download helper early for use throughout the build
COPY download_github_asset.sh /usr/local/bin/download_github_asset.sh
RUN chmod +x /usr/local/bin/download_github_asset.sh

# Install unblob dependencies, curl, and fakeroot
ENV DEBIAN_FRONTEND=noninteractive
ENV TZ=America/New_York
ENV LC_ALL=C.UTF-8
ENV LANG=C.UTF-8
ENV HOME=/root
ENV FW2TAR_LOG=warn
ENV FW2TAR_LOG_STYLE=always

# Debug: Show if GitHub token is available (using BuildKit secrets)
RUN --mount=type=secret,id=github_token \
    if [ -s /run/secrets/github_token ]; then \
        echo "GitHub token is available for authenticated downloads"; \
    else \
        echo "No GitHub token provided - using unauthenticated downloads"; \
    fi

RUN apt-get update && \
  apt-get install -q -y \
    android-sdk-libsparse-utils \
    arj \
    automake \
    build-essential \
    bzip2 \
    cabextract \
    clang \
    cpio \
    cramfsswap \
    curl \
    default-jdk \
    e2fsprogs \
    fakeroot \
    gcc \
    git \
    gzip \
    lhasa \
    libarchive-dev \
    libfontconfig1-dev \
    liblzma-dev \
    liblzo2-dev \
    liblz4-dev \
    libbz2-dev \
    libssl-dev \
    libmagic1 \
    locales \
    lz4 \
    lziprecover \
    lzop \
    mtd-utils \
    openssh-client \
    p7zip \
    p7zip-full \
    patchelf \
    python3 \
    python3-pip \
    qtbase5-dev \
    sleuthkit \
    squashfs-tools \
    srecord \
    tar \
    unrar \
    unrar-free \
    unyaffs\
    unzip \
    wget \
    xz-utils \
    zlib1g-dev \
    zstd

# Install dependencies
RUN pip install --upgrade pip && \
    python3 -m pip install \
      git+http://github.com/jrspruitt/ubi_reader.git@v0.8.5-master \
      git+https://github.com/rehosting/binwalk.git \
      git+https://github.com/ahupp/python-magic \
      git+https://github.com/devttys0/yaffshiv.git \
      git+https://github.com/marin-m/vmlinux-to-elf \
      jefferson \
      gnupg \
      psycopg2-binary \
      pycryptodome \
      pylzma \
      pyyaml \
      setuptools \
      sqlalchemy \
      telnetlib3 \
      tk \
      lz4 \
      zstandard \
      pyelftools \
      lief && \
    python3 -m pip install python-lzo==1.14

RUN --mount=type=secret,id=github_token \
    GITHUB_TOKEN="$(cat /run/secrets/github_token 2>/dev/null || echo '')" \
    /usr/local/bin/download_github_asset.sh \
        "https://github.com/onekey-sec/sasquatch/releases/download/sasquatch-v4.5.1-4/sasquatch_1.0_$(dpkg --print-architecture).deb" \
        sasquatch_1.0.deb && \
    dpkg -i sasquatch_1.0.deb && \
    rm sasquatch_1.0.deb

# Binwalk v3 dependencies
RUN git clone --depth=1 https://github.com/ReFirmLabs/binwalk /binwalk && \
    cd /binwalk/dependencies && \
    sh -c ./ubuntu.sh

# CramFS no longer in apt - needed by binwalk
RUN git clone --depth=1 https://github.com/davidribyrne/cramfs.git /cramfs && \
   cd /cramfs && make && make install

# Install Rust with cache mount (required by unblob maturin build and binwalk v3)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    curl https://sh.rustup.rs -sSf | bash -s -- -y

# Add .cargo/bin to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Clone unblob fork then install with uv (upstream dropped poetry)
RUN git clone --depth=1 https://github.com/rehosting/unblob.git /unblob

# Drop [tool.uv.sources] git override for arpy — its sdist breaks setuptools discovery
RUN python3 <<'PY'
from pathlib import Path
p = Path("/unblob/pyproject.toml")
text = p.read_text()
marker = "[tool.uv.sources]"
if marker in text:
    start = text.index(marker)
    end = text.find("\n[", start + 1)
    p.write_text(text[:start] + (text[end:] if end != -1 else ""))
PY

# Install arpy from PyPI then patch with the onekey-sec fork fix (unblob#767)
RUN pip install uv 'arpy==2.3.0' && \
    curl -fsSL -o "$(python3 -c 'import arpy; print(arpy.__file__)')" \
      "https://raw.githubusercontent.com/onekey-sec/arpy/f6746c566b92a193bfe57edb312809b6299ddab1/arpy.py"

RUN cd /unblob && uv pip install --system --no-cache .

# Install binwalk v3 with cache mount
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install binwalk

# Install fw2tar with cache mount
COPY ./Cargo.toml ./Cargo.lock /fw2tar_src/
COPY ./src /fw2tar_src/src/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/fw2tar_src/target \
    cargo install --path /fw2tar_src

# Explicitly install unblob deps - mostly captured above, but some of the .debs get updated and installed via curl
RUN sh -xe /unblob/install-deps.sh

# We will run as other users (matching uid/gid to host), but binwalk has config files in /root/.config
# that need to be created and read at runtime. Only chmod that tree — not all of /root (cargo/rust is huge).
RUN mkdir -p /root/.config && chmod -R 777 /root/.config

# Try to install custom fakeroot. This is optional - we have regular fakeroot. If we're building
# with host SSH keys, we can do this, otherwise we'll just skip it
# Setup ssh keys for github.com
RUN mkdir -p -m 0600 ~/.ssh && ssh-keyscan github.com >> ~/.ssh/known_hosts
ARG SSH
RUN --mount=type=ssh git clone git@github.com:rehosting/fakeroot.git /fakeroot && \
    sed -i 's/^# deb-src/deb-src/' /etc/apt/sources.list && \
    apt-get update && apt-get build-dep -y fakeroot && \
    cd /fakeroot && ./bootstrap && ./configure && make && make install -k || true

# Copy wrapper script into container so we can copy out - note we don't put it on guest path
COPY ./xfs /usr/local/src/xfs_wrapper
# And add install helpers which generate shell commands to install it on host
COPY ./src/resources/banner.sh ./src/resources/xfs_install ./src/resources/xfs_install.local /usr/local/bin/
# Warn on interactive shell sessions and provide instructions for install
RUN echo '[ ! -z "$TERM" ] && [ -z "$NOBANNER" ] && /usr/local/bin/banner.sh' >> /etc/bash.bashrc

COPY src/fakeroot_xfs /usr/local/bin/fakeroot_xfs

CMD ["/usr/local/bin/banner.sh"]
