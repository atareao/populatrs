#!/bin/bash

# Colores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Funciones auxiliares
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Función para mostrar ayuda
show_help() {
    cat << EOF
Populatrs Build and Deploy Script

Usage: $0 [COMMAND]

Commands:
    build       Build the Docker image
    run         Run the application with docker-compose
    stop        Stop the running containers
    logs        Show container logs
    check       Validate configuration
    clean       Clean up containers and images
    help        Show this help message

Examples:
    $0 build       # Build the Docker image
    $0 run         # Start the application
    $0 logs        # View logs
    $0 stop        # Stop the application

Configuration:
    Copy config.example.json to config.json and edit with your settings.

EOF
}

# Función para verificar dependencias
check_dependencies() {
    log_info "Checking dependencies..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed or not in PATH"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        log_error "Docker Compose is not installed or not in PATH"
        exit 1
    fi
    
    log_success "All dependencies are available"
}

# Función para construir la imagen Docker
build_image() {
    log_info "Building Docker image..."
    
    if docker build -t populatrs:latest .; then
        log_success "Docker image built successfully"
    else
        log_error "Failed to build Docker image"
        exit 1
    fi
}

# Función para verificar la configuración
check_config() {
    log_info "Checking configuration..."
    
    if [ ! -f "config.json" ]; then
        log_warning "config.json not found. Creating from example..."
        if [ -f "config.example.json" ]; then
            cp config.example.json config.json
            log_warning "Please edit config.json with your actual credentials before running"
        else
            log_error "config.example.json not found"
            exit 1
        fi
    else
        log_success "Configuration file found"
    fi
    
    # Verificar que el archivo JSON sea válido
    if command -v jq &> /dev/null; then
        if jq empty config.json &> /dev/null; then
            log_success "Configuration file is valid JSON"
        else
            log_error "Configuration file contains invalid JSON"
            exit 1
        fi
    else
        log_warning "jq not installed, skipping JSON validation"
    fi
}

# Función para ejecutar la aplicación
run_app() {
    log_info "Starting Populatrs..."
    
    check_config
    
    # Crear directorio de datos si no existe
    mkdir -p data
    
    if docker-compose up -d; then
        log_success "Populatrs started successfully"
        log_info "Use '$0 logs' to view logs"
        log_info "Use '$0 stop' to stop the application"
    else
        log_error "Failed to start Populatrs"
        exit 1
    fi
}

# Función para parar la aplicación
stop_app() {
    log_info "Stopping Populatrs..."
    
    if docker-compose down; then
        log_success "Populatrs stopped successfully"
    else
        log_error "Failed to stop Populatrs"
        exit 1
    fi
}

# Función para mostrar logs
show_logs() {
    log_info "Showing Populatrs logs..."
    docker-compose logs -f
}

# Función para limpiar
clean_up() {
    log_info "Cleaning up containers and images..."
    
    docker-compose down --remove-orphans
    docker rmi populatrs:latest 2>/dev/null || true
    docker system prune -f
    
    log_success "Cleanup completed"
}

# Función principal
main() {
    case "${1:-help}" in
        "build")
            check_dependencies
            build_image
            ;;
        "run")
            check_dependencies
            run_app
            ;;
        "stop")
            stop_app
            ;;
        "logs")
            show_logs
            ;;
        "check")
            check_config
            ;;
        "clean")
            clean_up
            ;;
        "help")
            show_help
            ;;
        *)
            log_error "Unknown command: $1"
            show_help
            exit 1
            ;;
    esac
}

# Ejecutar función principal
main "$@"