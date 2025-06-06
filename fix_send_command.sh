#!/bin/bash

# Replace send_command with execute_command and add error handling
for file in src/*_ext.rs; do
    echo "Fixing $file..."
    # Replace simple send_command calls followed by Ok(())
    sed -i 's/self\.send_command(\(.*\))?;$/match self.execute_command(\1)? {\
            ViscaResponse::Completion => {},\
            ViscaResponse::Error(e) => return Err(e),\
            _ => return Err(ViscaError::UnexpectedResponseType),\
        }/g' "$file"
done

echo "Done. You may need to manually add ViscaResponse to imports."