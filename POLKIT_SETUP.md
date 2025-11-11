# PolicyKit Setup for PackageKit Integration

If you encounter permission errors when using PackageKit operations (like `flux package status`), you may need to configure PolicyKit rules.

## Error Messages

You might see errors like:

- `org.freedesktop.DBus.Error.UnknownMethod: No such method "GetPackages"`
- `PackageKit permission denied`
- `sender does not match`

## Solutions

### Option 1: Run with sudo (Quick Fix)

For one-time operations, you can run with elevated privileges:

```bash
sudo flux package status
sudo flux apply
```

### Option 2: Create PolicyKit Rule (Recommended for Regular Use)

Create a PolicyKit rule file to allow your user to use PackageKit without sudo:

1. Create the rules file:

   ```bash
   sudo nano /etc/polkit-1/rules.d/99-flux-packagekit.rules
   ```

2. Add the following content:

   ```javascript
   polkit.addRule(function(action, subject) {
       // Allow query operations (reading package lists)
       if (action.id == "org.freedesktop.packagekit.package-install" ||
           action.id == "org.freedesktop.packagekit.package-remove" ||
           action.id == "org.freedesktop.packagekit.package-update") {
           // Allow for users in the wheel group (adjust as needed)
           if (subject.is_in_group("wheel")) {
               return polkit.Result.YES;
           }
       }
       // For query operations, PackageKit might not require explicit actions
       // but may still need proper D-Bus permissions
   });
   ```

3. Verify the rule is loaded:

   ```bash
   pkaction | grep packagekit
   ```

### Option 3: Check PackageKit Service Status

Ensure PackageKit is running:

```bash
systemctl status packagekit
```

If it's not running, start it:

```bash
sudo systemctl start packagekit
```

## Available PackageKit Actions

To see all available PolicyKit actions for PackageKit:

```bash
pkaction | grep packagekit
```

Common actions you might need:

- `org.freedesktop.packagekit.package-install` - Install packages
- `org.freedesktop.packagekit.package-remove` - Remove packages
- `org.freedesktop.packagekit.package-update` - Update packages

## Troubleshooting

1. **Check D-Bus permissions**: Ensure your user can access the system D-Bus
2. **Verify PolicyKit is working**: Test with `pkaction` command
3. **Check logs**: Look at `journalctl -u packagekit` for detailed error messages
4. **Test with pkcon**: Try `pkcon get-packages --filter installed` to verify PackageKit works

## Security Note

PolicyKit rules grant system-level permissions. Only create rules for trusted applications and users. The example above restricts access to users in the `wheel` group (typically administrators).
