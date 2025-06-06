#!/bin/bash

# List of extension trait files to update
ext_files=(
    "src/exposure_ext.rs"
    "src/focus_ext.rs"
    "src/image_ext.rs"
    "src/inquiry_ext.rs"
    "src/pan_tilt_ext.rs"
    "src/position_ext.rs"
    "src/power_ext.rs"
    "src/preset_ext.rs"
    "src/white_balance_ext.rs"
)

for file in "${ext_files[@]}"; do
    echo "Updating $file..."
    
    # Replace ViscaTransport with ViscaDevice in imports
    sed -i 's/ViscaTransport,/ViscaDevice,/g' "$file"
    
    # Replace ViscaTransportExt with ViscaDevice in trait definitions
    sed -i 's/: ViscaTransportExt/: ViscaDevice/g' "$file"
    
    # Replace send_and_wait with execute_command
    sed -i 's/self\.send_and_wait/self.execute_command/g' "$file"
    
    # Replace send_command with execute_command (but need to handle the response)
    # This is more complex and would need manual review
    
    # Update examples from transport to client
    sed -i 's/transport\./client\./g' "$file"
    sed -i 's/mut transport/mut client/g' "$file"
    sed -i 's/impl ViscaTransport/&mut ViscaClient/g' "$file"
    sed -i 's/UdpTransport::new/ViscaClient::connect_udp/g' "$file"
    
    # Remove old blanket implementations
    sed -i '/impl<T: ViscaTransportExt>/,/^}$/d' "$file"
    sed -i '/impl<T: ViscaTransport.*ViscaTransportExt/,/^}$/d' "$file"
done

echo "Extension traits updated. Manual review recommended for:"
echo "1. send_command calls that need proper error handling"
echo "2. Adding blanket implementations for ViscaDevice"
echo "3. Updating documentation examples"