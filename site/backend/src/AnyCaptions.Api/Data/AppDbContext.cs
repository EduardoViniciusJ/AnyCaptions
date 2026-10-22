using AnyCaptions.Api.Domain.Entities;
using Microsoft.AspNetCore.DataProtection.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore;

namespace AnyCaptions.Api.Data;

public sealed class AppDbContext(DbContextOptions<AppDbContext> options) : DbContext(options), IDataProtectionKeyContext
{
    public DbSet<User> Users => Set<User>();
    public DbSet<UserLogin> UserLogins => Set<UserLogin>();
    public DbSet<Release> Releases => Set<Release>();
    public DbSet<DataProtectionKey> DataProtectionKeys => Set<DataProtectionKey>();

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.Entity<User>(entity =>
        {
            entity.HasKey(user => user.Id);
            entity.Property(user => user.Name).HasMaxLength(200);
            entity.Property(user => user.Email).HasMaxLength(320);
            entity.Property(user => user.AvatarUrl).HasMaxLength(2048);
        });
        modelBuilder.Entity<UserLogin>(entity =>
        {
            entity.HasKey(login => login.Id);
            entity.HasIndex(login => new { login.Provider, login.ProviderKey }).IsUnique();
            entity.Property(login => login.Provider).HasMaxLength(32);
            entity.Property(login => login.ProviderKey).HasMaxLength(256);
            entity.HasOne(login => login.User).WithMany(user => user.Logins).HasForeignKey(login => login.UserId);
        });
        modelBuilder.Entity<Release>(entity =>
        {
            entity.HasKey(release => release.Id);
            entity.HasIndex(release => release.Version).IsUnique();
            entity.Property(release => release.Version).HasMaxLength(64);
            entity.Property(release => release.WindowsInstallerUrl).HasMaxLength(2048);
            entity.Property(release => release.WindowsZipUrl).HasMaxLength(2048);
            entity.Property(release => release.Sha256).HasMaxLength(64);
        });
    }
}
