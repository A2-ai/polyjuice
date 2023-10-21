#!/bin/bash

# Check if the script is running as root
if [[ $EUID -ne 0 ]]; then
    echo "This script must be run as root."
    exit 1
fi

# Define the starting UID
START_UID=100000

# Loop to create 10 users
for ((i=0; i<10; i++)); do
    # Calculate the UID for the current user
    USER_UID=$((START_UID + i))

    # Create the user without a home directory and set the login shell to /bin/bash
    useradd -M -s /bin/bash -u $USER_UID user$i

    # Display information about the created user
    echo "User user$i created with UID $USER_UID and login shell /bin/bash"
done

# Display a message when the script is done
echo "User creation completed."
